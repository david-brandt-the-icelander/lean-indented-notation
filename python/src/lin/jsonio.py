"""Strict JSON in, the same JSON out: every LIN implementation writes the same JSON text, and numbers keep the text they were read with."""
import json

from .core import JsonFloat, JsonInt, LinError, is_number, json_string, num


def _pairs(pairs):
    o = {}
    for k, v in pairs:
        if k in o:
            raise LinError(f'duplicate key {k!r} in JSON')
        o[k] = v
    return o


def _no_constant(name):
    raise LinError(f'{name} is not JSON')


def from_json(text):
    """Parse JSON strictly: duplicate keys and NaN/Infinity are errors; numbers keep their text."""
    try:
        return json.loads(text, object_pairs_hook=_pairs, parse_constant=_no_constant,
                          parse_int=JsonInt, parse_float=JsonFloat)
    except json.JSONDecodeError as e:
        raise LinError(f'bad JSON: {e.msg} at line {e.lineno} column {e.colno}') from None


def to_json(v, indent=2):
    """JSON text, each number as it was read (others in shortest form). indent=None gives compact JSON on one line;
    otherwise the layout of json.dumps(indent=2) and JSON.stringify(v, null, 2), plus a final newline."""
    if indent is None:
        return _compact(v)
    out = []
    _pretty(v, 0, indent, out)
    return ''.join(out) + '\n'


def _scalar(v):
    if v is None:
        return 'null'
    if v is True:
        return 'true'
    if v is False:
        return 'false'
    if is_number(v):
        return num(v)
    if isinstance(v, str):
        return json_string(v)
    raise TypeError(f'not a JSON value: {type(v).__name__}')


def _compact(v):
    if isinstance(v, dict):
        return '{' + ','.join(json_string(k) + ':' + _compact(x) for k, x in v.items()) + '}'
    if isinstance(v, list):
        return '[' + ','.join(_compact(x) for x in v) + ']'
    return _scalar(v)


def _pretty(v, level, ind, out):
    if isinstance(v, dict):
        if not v:
            out.append('{}')
            return
        out.append('{')
        for i, (k, x) in enumerate(v.items()):
            out.append(('\n' if i == 0 else ',\n') + ' ' * (ind * (level + 1)) + json_string(k) + ': ')
            _pretty(x, level + 1, ind, out)
        out.append('\n' + ' ' * (ind * level) + '}')
    elif isinstance(v, list):
        if not v:
            out.append('[]')
            return
        out.append('[')
        for i, x in enumerate(v):
            out.append(('\n' if i == 0 else ',\n') + ' ' * (ind * (level + 1)))
            _pretty(x, level + 1, ind, out)
        out.append('\n' + ' ' * (ind * level) + ']')
    else:
        out.append(_scalar(v))
