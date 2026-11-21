"""LIN 1 reader and writer.

Values are the ones Python's json module uses: dict, list, str, int, float, bool and None.
"""
import json
import math
import re

VERSION = '1'
IND = ' '  # indentation: one space per level

KEY_RE = r'[^\s"\[\]{}|+#-][^\s"\[\]{}|]*'
BARE_KEY = re.compile(KEY_RE + r'\Z')
KEY_AT = re.compile(KEY_RE)
NUM = re.compile(r'-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?\Z')
INT_TEXT = re.compile(r'-?\d+\Z')
REGISTERED_ID = re.compile(r'[1-9][0-9]{0,8}\Z')   # assigned by the official registry
PROVISIONAL_ID = re.compile(r'[0-9]{39}\Z')         # 128 bits of the schema's SHA-256, in decimal


def schema_id_kind(s):
    """'registered', 'provisional', or None if s is not a schema id."""
    if REGISTERED_ID.match(s):
        return 'registered'
    if PROVISIONAL_ID.match(s):
        return 'provisional'
    return None
SEG = re.compile(r'[^.\s"|@]+')
HEADLINE = frozenset({'id', 'kind', 'title', 'name', 'status', 'type'})
_BAD = frozenset('\x7f\x85  ')

DEFAULT_SPLIT_LIMIT = 24
DEFAULT_OBJECT_LIMIT = 80


class LinError(ValueError):
    """A LIN text breaks a rule of the spec. `line` is 1-based when known."""

    def __init__(self, message, line=None):
        super().__init__(f'line {line}: {message}' if line else message)
        self.message, self.line = message, line


# ---------------------------------------------------------------- scalars

def bad_char(s):
    return any(ord(c) < 0x20 or c in _BAD for c in s)


def bare_key_ok(k):
    return bool(BARE_KEY.match(k)) and not bad_char(k)


def quote_key(k):
    return k if bare_key_ok(k) else json_string(k)


def json_string(s):
    """A JSON string as JSON.stringify writes it (non-ASCII kept)."""
    return json.dumps(s, ensure_ascii=False)


def num(x):
    """A number in the shortest form JSON produces: integers exactly, floats as
    ECMAScript Number::toString (1.0 -> 1, 1e-05 -> 0.00001, 1e21 -> 1e+21)."""
    if isinstance(x, bool):
        raise TypeError('bool is not a number')
    if isinstance(x, int):
        return str(x)
    if not math.isfinite(x):
        raise ValueError('LIN numbers must be finite')
    if x == 0:
        return '0'
    sign = '-' if x < 0 else ''
    m, _, e = repr(abs(x)).partition('e')
    ip, _, fp = m.partition('.')
    fp = '' if fp == '0' else fp
    exp = int(e) if e else 0
    if ip.strip('0'):
        n = len(ip.lstrip('0')) + exp
    else:
        n = -(len(fp) - len(fp.lstrip('0'))) + exp
    digits = ((ip + fp).lstrip('0') or '0').rstrip('0') or '0'
    k = len(digits)
    if k <= n <= 21:
        r = digits + '0' * (n - k)
    elif 0 < n <= 21:
        r = digits[:n] + '.' + digits[n:]
    elif -6 < n <= 0:
        r = '0.' + '0' * -n + digits
    else:
        ex = n - 1
        r = digits[0] + ('.' + digits[1:] if k > 1 else '') + 'e' + ('+' if ex > 0 else '-') + str(abs(ex))
    return sign + r


def parse_number(text):
    """JSON number text to int or float; floats must be finite."""
    if re.fullmatch(r'-?\d+', text):
        return int(text)
    v = float(text)
    if not math.isfinite(v):
        raise LinError(f'number out of range: {text}')
    return v


def is_number(v):
    return isinstance(v, (int, float)) and not isinstance(v, bool)


def id_segment(v):
    """The name segment an id gives its object, or None if it cannot be an owner."""
    if isinstance(v, str):
        s = v
    elif is_number(v):
        s = num(v)
        if not INT_TEXT.match(s):
            return None
    else:
        return None
    return s if seg_ok(s) else None


def seg_ok(s):
    return bool(s) and not re.search(r'[.\s"|@]', s) and not bad_char(s)


def clen(s):
    """Length in characters (Unicode code points), as every limit counts it."""
    return len(s)


# ------------------------------------------------------------- schema typing

ALL_TYPES = frozenset({'null', 'boolean', 'number', 'string', 'array', 'object'})


class SchemaTypes:
    """The subset of JSON Schema that decides how bare scalars are typed (spec section 7).

    types(node) is the set of JSON types a position allows; prop(node, key) and item(node, i)
    give the child positions. Unknown constructs allow every type, which is the schema-less rule.
    """

    def __init__(self, schema):
        self.root = schema

    def _ref(self, node):
        seen = 0
        while isinstance(node, dict) and isinstance(node.get('$ref'), str) and node['$ref'].startswith('#') and seen < 64:
            target = self._pointer(node['$ref'][1:])
            if target is None:
                break
            rest = {k: v for k, v in node.items() if k != '$ref'}
            if rest:
                return {'allOf': [target, rest]}
            node, seen = target, seen + 1
        return node

    def _pointer(self, ptr):
        cur = self.root
        if ptr == '':
            return cur
        if not ptr.startswith('/'):
            return None
        for part in ptr[1:].split('/'):
            part = part.replace('~1', '/').replace('~0', '~')
            if isinstance(cur, dict) and part in cur:
                cur = cur[part]
            elif isinstance(cur, list) and part.isdigit() and int(part) < len(cur):
                cur = cur[int(part)]
            else:
                return None
        return cur

    def types(self, node):
        node = self._ref(node)
        if not isinstance(node, dict):
            return ALL_TYPES
        out = ALL_TYPES
        t = node.get('type')
        if isinstance(t, str):
            t = [t]
        if isinstance(t, list):
            out = out & frozenset('number' if x == 'integer' else x for x in t if isinstance(x, str))
        if 'const' in node:
            out = out & frozenset({type_of(node['const'])})
        if isinstance(node.get('enum'), list):
            out = out & frozenset(type_of(x) for x in node['enum'])
        for sub in node.get('allOf', []) if isinstance(node.get('allOf'), list) else []:
            out = out & self.types(sub)
        for key in ('anyOf', 'oneOf'):
            if isinstance(node.get(key), list) and node[key]:
                u = frozenset()
                for sub in node[key]:
                    u = u | self.types(sub)
                out = out & u
        return out

    def _children(self, node, pick):
        """Combine child positions: allOf intersects (a list of nodes), anyOf/oneOf unions."""
        node = self._ref(node)
        if not isinstance(node, dict):
            return [True]
        own = pick(node)
        alls = [own]
        for sub in node.get('allOf', []) if isinstance(node.get('allOf'), list) else []:
            alls.append(self._children(sub, pick))
        for key in ('anyOf', 'oneOf'):
            if isinstance(node.get(key), list) and node[key]:
                alls.append({'anyOf': [{'allOf': self._children(sub, pick)} for sub in node[key]]})
        flat = []
        for a in alls:
            flat.extend(a if isinstance(a, list) else [a])
        return flat

    def prop(self, node, key):
        def pick(n):
            props = n.get('properties')
            if isinstance(props, dict) and key in props:
                return props[key]
            pats = n.get('patternProperties')
            if isinstance(pats, dict):
                hits = [s for p, s in pats.items() if _search(p, key)]
                if hits:
                    return {'allOf': hits}
            if 'additionalProperties' in n:
                return n['additionalProperties']
            return True
        return {'allOf': self._children(node, pick)}

    def item(self, node, index):
        def pick(n):
            pre = n.get('prefixItems')
            if isinstance(pre, list) and index < len(pre):
                return pre[index]
            if 'items' in n and not isinstance(n['items'], list):
                return n['items']
            return True
        return {'allOf': self._children(node, pick)}


def _search(pattern, s):
    try:
        return re.search(pattern, s) is not None
    except re.error:
        return False


def type_of(v):
    if v is None:
        return 'null'
    if isinstance(v, bool):
        return 'boolean'
    if is_number(v):
        return 'number'
    if isinstance(v, str):
        return 'string'
    if isinstance(v, list):
        return 'array'
    return 'object'


def bare_reads_as_string(s, types):
    """Would bare text s, at a position allowing `types`, read back as the string s?"""
    if NUM.match(s):
        return 'number' not in types and 'string' in types
    if s in ('true', 'false'):
        return 'boolean' not in types and 'string' in types
    return True


class _Typer:
    """Walks a value with its schema positions. Without a schema every position allows all types."""

    def __init__(self, schema):
        self.st = SchemaTypes(schema) if schema is not None else None

    def types(self, node):
        return ALL_TYPES if self.st is None else self.st.types(node)

    def prop(self, node, key):
        return None if self.st is None else self.st.prop(node, key)

    def item(self, node, i):
        return None if self.st is None else self.st.item(node, i)

    def root(self):
        return None if self.st is None else self.st.root


# ------------------------------------------------------------------ writer

class _Ref:
    __slots__ = ('name', 'value')

    def __init__(self, name=None, value=None):
        self.name, self.value = name, value  # name: tuple of segments, or None = derived


class _Str(str):
    """A string that may be written bare although it looks like a number or boolean,
    because the schema says that position is a string."""


def _mark(value, typer, node):
    if isinstance(value, dict):
        return {k: _mark(v, typer, typer.prop(node, k)) for k, v in value.items()}
    if isinstance(value, list):
        return [_mark(v, typer, typer.item(node, i)) for i, v in enumerate(value)]
    if isinstance(value, str) and type(value) is str and (NUM.match(value) or value in ('true', 'false')):
        if bare_reads_as_string(value, typer.types(node)):
            return _Str(value)
    return value


class Writer:
    def __init__(self, split_limit=DEFAULT_SPLIT_LIMIT, inline_limit=None, object_limit=DEFAULT_OBJECT_LIMIT):
        self.split_limit = split_limit
        self.inline_limit = inline_limit  # None = every list of scalars goes inline
        self.object_limit = object_limit  # 0 or None = no inline objects

    # -- scalars
    def scalar(self, v, cell=False):
        """The one-line form of a scalar, or None when it needs a block."""
        if isinstance(v, _Ref):
            return '@' if v.name is None else '@' + '.'.join(v.name)
        if v is None:
            return '~'
        if v is True:
            return 'true'
        if v is False:
            return 'false'
        if is_number(v):
            return num(v)
        s = v
        if '\n' in s and not cell and block_ok(s):
            return None
        looks = (NUM.match(s) or s in ('true', 'false')) and not isinstance(s, _Str)
        need = (s == '' or s[0] in ' "@' or s[-1] == ' ' or s in ('~', '[]', '{}', '|')
                or bad_char(s) or looks or (cell and '|' in s))
        return json_string(str(s)) if need else str(s)

    def cells(self, values):
        return '|'.join(self.scalar(x, cell=True) for x in values)

    # -- containers
    def inline_ok(self, lst):
        if not lst:
            return False
        for x in lst:
            if isinstance(x, (dict, list, _Ref)):
                return False
            if isinstance(x, str) and self.inline_limit is not None and clen(x) > self.inline_limit:
                return False
        return True

    def obj_cells(self, o):
        return '|'.join(f'{k} {self.scalar(v, cell=True)}' for k, v in o.items())

    def inline_obj_ok(self, o, ind, item=False):
        if not self.object_limit or ind < 1 or len(o) < (2 if item else 1):
            return False
        if not all(not isinstance(v, (dict, list)) and bare_key_ok(k) for k, v in o.items()):
            return False
        return clen(self.obj_cells(o)) <= self.object_limit

    @staticmethod
    def table_cols(lst):
        if len(lst) < 2 or not all(isinstance(x, dict) and x for x in lst):
            return None
        keys = list(lst[0])
        if not all(bare_key_ok(k) for k in keys):
            return None
        for x in lst:
            if list(x) != keys or any(isinstance(v, (dict, list)) for v in x.values()):
                return None
        return keys

    def emit(self, o, ind):
        p = IND * ind
        out = []
        for k, v in o.items():
            kk = quote_key(k)
            if isinstance(v, dict):
                if not v:
                    out.append(f'{p}{kk} {{}}')
                elif self.inline_obj_ok(v, ind):
                    out.append(f'{p}{kk}{{}} ' + self.obj_cells(v))
                else:
                    out.append(p + kk)
                    out += self.emit(v, ind + 1)
            elif isinstance(v, list):
                cols = self.table_cols(v)
                if not v:
                    out.append(f'{p}{kk} []')
                elif self.inline_ok(v):
                    out.append(f'{p}{kk}[] ' + self.cells(v))
                elif cols:
                    out.append(f'{p}{kk}[{"|".join(cols)}]')
                    out += [p + IND + self.cells([x[c] for c in cols]) for x in v]
                else:
                    out.append(p + kk)
                    out += self.emit_items(v, ind)
            else:
                s = self.scalar(v)
                if s is None:
                    out.append(f'{p}{kk} |')
                    out += block_lines(v, ind + 1)
                else:
                    out.append(f'{p}{kk} {s}')
        return out

    def emit_items(self, lst, ind):
        p = IND * ind
        out = []
        for x in lst:
            if isinstance(x, dict):
                if not x:
                    out.append(p + '- {}')
                elif self.inline_obj_ok(x, ind, item=True):
                    out.append(p + '-{} ' + self.obj_cells(x))
                else:
                    sub = self.emit(x, ind + 1)
                    out.append(p + '+ ' + sub[0][len(p) + len(IND):])
                    out += sub[1:]
            elif isinstance(x, list):
                if not x:
                    out.append(p + '- []')
                elif self.inline_ok(x):
                    out.append(p + '-[] ' + self.cells(x))
                else:
                    out.append(p + '-')
                    out += self.emit_items(x, ind + 1)
            else:
                s = self.scalar(x)
                if s is None:
                    out.append(p + '- |')
                    out += block_lines(x, ind + 1)
                else:
                    out.append(p + '- ' + s)
        return out

    def body(self, doc):
        if isinstance(doc, dict):
            return self.emit(doc, 0)
        if not doc:
            return ['[]']
        if self.inline_ok(doc):
            return ['[] ' + self.cells(doc)]
        cols = self.table_cols(doc)
        if cols:
            return ['[' + '|'.join(cols) + ']'] + [IND + self.cells([x[c] for c in cols]) for x in doc]
        return self.emit_items(doc, 0)

    # -- split form
    def split(self, doc):
        lim = self.split_limit

        def long_str(x):
            return isinstance(x, str) and clen(x) > lim

        def move(o, key, root=False):
            if isinstance(o, dict):
                return {k: move(v, k) for k, v in o.items()}
            if isinstance(o, list):
                if (not root and o and all(not isinstance(x, (dict, list)) for x in o)
                        and any(long_str(x) for x in o)):
                    return _Ref(None, o)
                return [move(x, None) for x in o]
            if long_str(o) and key not in HEADLINE:
                return _Ref(None, o)
            return o

        head = move(doc, None, root=True)
        refs = []
        walk_names(head, lambda c, k, v, n: refs.append((c, k, v, n)) if isinstance(v, _Ref) else None)
        derived = {}
        for _, _, _, n in refs:
            derived[n] = derived.get(n, 0) + 1
        used, data = set(), []
        for c, k, v, n in refs:
            if not all(seg_ok(x) for x in n):
                c[k] = v.value  # its name would need a forbidden character: it stays in the head
                continue
            if n not in used and derived[n] == 1:
                name = n
                c[k] = _Ref(None, v.value)
            else:
                i = 2
                while True:
                    name = n[:-1] + (f'{n[-1]}~{i}',)
                    if name not in derived and name not in used:
                        break
                    i += 1
                c[k] = _Ref(name, v.value)
            used.add(name)
            data.append((name, v.value))
        return head, data

    def data_lines(self, data):
        out = []
        for name, v in data:
            n = '@' + '.'.join(name)
            if isinstance(v, list):
                out.append(n)
                out += self.emit_items(v, 1)
            else:
                s = self.scalar(v)
                if s is None:
                    out.append(n + ' |')
                    out += block_lines(v, 1)
                else:
                    out.append(f'{n} {s}')
        return out


def block_ok(s):
    lines = s.split('\n')
    return (not s.endswith('\n') and lines[0] != '' and all(l == l.rstrip(' ') for l in lines)
            and not any(bad_char(l) for l in lines))


def block_lines(s, ind):
    p = IND * ind
    return [p + l if l else '' for l in s.split('\n')]


def walk_names(o, visit, owner=(), path=()):
    """Visit every value in document order with the reference name it would get (spec section 8)."""
    if isinstance(o, dict):
        seg = id_segment(o.get('id')) if 'id' in o else None
        if seg is not None:
            owner, path = (seg,), ()
        for k, v in list(o.items()):
            visit(o, k, v, owner + path + (k,))
            walk_names(v, visit, owner, path + (k,))
    elif isinstance(o, list):
        for i, v in enumerate(list(o)):
            visit(o, i, v, owner + path + (str(i),))
            walk_names(v, visit, owner, path + (str(i),))


def header_line(schema=None, url=None):
    h = 'LIN ' + VERSION
    if schema:
        kind = schema_id_kind(schema)
        if kind is None:
            raise ValueError('a schema id is a registered number (1 to 9 digits) or a 39-digit provisional id')
        h += ' ' + schema
        if url and kind != 'provisional':
            raise ValueError('a url may follow only a provisional schema id')
        if url:
            if not url.startswith('https://') or ' ' in url:
                raise ValueError('schema url must start with https:// and have no spaces')
            h += ' ' + url
    elif url:
        raise ValueError('a url needs a schema id before it')
    return h


def dumps(doc, *, split=False, schema=None, url=None, schema_doc=None,
          split_limit=DEFAULT_SPLIT_LIMIT, inline_limit=None, object_limit=DEFAULT_OBJECT_LIMIT,
          header=True):
    """Write a JSON value (object or array) as LIN.

    schema: the header's schema hash; schema_doc: the schema itself, which lets strings that
    look like numbers or booleans go bare where the schema says they are strings.
    """
    if not isinstance(doc, (dict, list)):
        raise ValueError('the root must be an object or an array')
    check_value(doc)
    if schema_doc is not None:
        doc = _mark(doc, _Typer(schema_doc), schema_doc)
    w = Writer(split_limit, inline_limit, object_limit)
    data = []
    if split:
        doc, data = w.split(doc)
    lines = ([header_line(schema, url)] if header else []) + w.body(doc)
    if data:
        lines.append('---')
        lines += w.data_lines(data)
    return '\n'.join(lines) + '\n'


def dumps_fragment(value, **kw):
    """A value written as LIN without a header line: a body for an object or array, a scalar's line form otherwise."""
    if isinstance(value, (dict, list)):
        return dumps(value, header=False, **kw)
    s = Writer().scalar(value)
    return (s if s is not None else json_string(value)) + '\n'


def check_value(v, depth=0):
    if isinstance(v, dict):
        for k, x in v.items():
            if not isinstance(k, str):
                raise TypeError('object keys must be strings')
            check_value(x, depth + 1)
    elif isinstance(v, list):
        for x in v:
            check_value(x, depth + 1)
    elif isinstance(v, float) and not math.isfinite(v):
        raise ValueError('LIN numbers must be finite')
    elif not (v is None or isinstance(v, (str, bool, int, float))):
        raise TypeError(f'not a JSON value: {type(v).__name__}')


# ------------------------------------------------------------------ reader

class _Bare(str):
    """Bare text with number or boolean syntax; its type is decided by the schema."""


_DECODER = json.JSONDecoder(strict=True)


def _json_str_at(s, i, line):
    try:
        v, j = _DECODER.raw_decode(s, i)
    except json.JSONDecodeError as e:
        raise LinError(f'bad JSON string: {e.msg}', line) from None
    if not isinstance(v, str):
        raise LinError('expected a JSON string', line)
    return v, j


def _bare_value(s, line, cell):
    if s == '':
        raise LinError('empty value; an empty string is written ""' if not cell else 'empty cell; an empty string is written ""', line)
    if s[0] == ' ' or s[-1] == ' ':
        raise LinError(f'value {s!r} starts or ends with a space; such a string is written as a JSON string', line)
    if bad_char(s):
        raise LinError('a control character in a bare value; write it as a JSON string', line)
    if s == '~':
        return None
    if s == '[]':
        return []
    if s == '{}':
        return {}
    if NUM.match(s) or s in ('true', 'false'):
        return _Bare(s)
    if s[0] == '@':
        return _Ref(parse_name(s[1:], line) if len(s) > 1 else None)
    return s


def parse_scalar(s, line=None, cell=False):
    if s.startswith('"'):
        v, end = _json_str_at(s, 0, line)
        if end != len(s):
            raise LinError(f'text after a JSON string: {s[end:]!r}', line)
        return v
    return _bare_value(s, line, cell)


def parse_name(s, line=None):
    segs, i = [], 0
    while True:
        m = SEG.match(s, i)
        if not m:
            raise LinError(f'bad reference name {s!r}', line)
        segs.append(m.group())
        i = m.end()
        if i == len(s):
            return tuple(segs)
        if s[i] != '.':
            raise LinError(f'bad reference name {s!r}', line)
        i += 1


def split_cells(s, line):
    cells, i = [], 0
    while True:
        if s[i:i + 1] == '"':
            v, j = _json_str_at(s, i, line)
        else:
            j = s.find('|', i)
            j = len(s) if j < 0 else j
            v = _bare_value(s[i:j], line, cell=True)
        cells.append(v)
        if j == len(s):
            return cells
        if s[j] != '|':
            raise LinError(f'expected | after a cell, found {s[j:]!r}', line)
        i = j + 1


def obj_from_cells(s, line):
    o, i = {}, 0
    while True:
        if s[i:i + 1] == '"':
            k, i = _json_str_at(s, i, line)
        else:
            m = KEY_AT.match(s, i)
            if not m:
                raise LinError(f'bad key in inline object: {s[i:]!r}', line)
            k, i = m.group(), m.end()
        if s[i:i + 1] != ' ':
            raise LinError(f'inline object field {k!r} needs one space and a value', line)
        i += 1
        if s[i:i + 1] == '"':
            v, j = _json_str_at(s, i, line)
        else:
            j = s.find('|', i)
            j = len(s) if j < 0 else j
            v = _bare_value(s[i:j], line, cell=True)
        if k in o:
            raise LinError(f'duplicate key {k!r}', line)
        o[k] = v
        if j == len(s):
            return o
        if s[j] != '|':
            raise LinError(f'expected | after a field, found {s[j:]!r}', line)
        i = j + 1


def _columns(spec, line):
    cols = spec.split('|')
    for c in cols:
        if not bare_key_ok(c):
            raise LinError(f'bad column name {c!r}: columns are bare keys', line)
    if len(set(cols)) != len(cols):
        raise LinError('duplicate column name', line)
    return cols


class _Reader:
    def __init__(self, lines, first_no):
        self.l, self.i, self.base = lines, 0, first_no

    def no(self, i=None):
        return self.base + (self.i if i is None else i)

    def peek(self):
        while self.i < len(self.l) and self.l[self.i].strip(' ') == '':
            self.i += 1
        return self.l[self.i] if self.i < len(self.l) else None

    @staticmethod
    def indent(line):
        return len(line) - len(line.lstrip(' '))

    def block(self, ind):
        p, out = IND * ind, []
        while self.i < len(self.l):
            line = self.l[self.i]
            if line == '':
                out.append('')
            elif line.startswith(p):
                out.append(line[len(p):])
            else:
                break
            self.i += 1
        while out and out[-1] == '':
            out.pop()
            self.i -= 1
        if not out:
            raise LinError('empty multi-line value', self.no())
        return '\n'.join(out)

    def value_after(self, rest, ind, line):
        if rest == '|':
            return self.block(ind + 1)
        return parse_scalar(rest, line)

    def table_rows(self, cols, ind, line):
        rows = []
        while True:
            nxt = self.peek()
            if nxt is None or self.indent(nxt) != ind:
                break
            cells = split_cells(nxt[ind:], self.no())
            if len(cells) != len(cols):
                raise LinError(f'row has {len(cells)} cells, the header has {len(cols)} columns', self.no())
            self.i += 1
            rows.append(dict(zip(cols, cells)))
        if not rows:
            raise LinError('table with no rows', line)
        return rows

    def obj(self, ind, first=None, first_no=None):
        o = {}
        while True:
            if first is not None:
                line, no, first = first, first_no, None
            else:
                raw = self.peek()
                if raw is None:
                    return o
                n = self.indent(raw)
                if n != ind:
                    if n > ind:
                        raise LinError('unexpected indentation', self.no())
                    return o
                line, no = raw[ind:], self.no()
                self.i += 1
            if line[:1] == '"':
                k, j = _json_str_at(line, 0, no)
                rest = line[j:]
            else:
                m = KEY_AT.match(line)
                if not m:
                    raise LinError(f'expected a key, found {line!r}', no)
                k, rest = m.group(), line[m.end():]
                if bad_char(k):
                    raise LinError('a control character in a bare key', no)
            if k in o:
                raise LinError(f'duplicate key {k!r}', no)
            if rest == '':
                nxt = self.peek()
                if nxt is not None and self.indent(nxt) == ind and nxt[ind:ind + 1] in '-+':
                    o[k] = self.items(ind)
                elif nxt is not None and self.indent(nxt) == ind + 1:
                    o[k] = self.obj(ind + 1)
                else:
                    raise LinError(f'key {k!r} has no value and no children', no)
            elif rest.startswith('[] '):
                o[k] = split_cells(rest[3:], no)
            elif rest.startswith('{} '):
                o[k] = obj_from_cells(rest[3:], no)
            elif rest[0] == '[' and rest.endswith(']') and len(rest) > 2:
                o[k] = self.table_rows(_columns(rest[1:-1], no), ind + 1, no)
            elif rest[0] == ' ':
                o[k] = self.value_after(rest[1:], ind, no)
            else:
                raise LinError(f'expected a space, [] , {{}} or a table header after key {k!r}', no)

    def items(self, ind):
        out = []
        while True:
            raw = self.peek()
            if raw is None or self.indent(raw) != ind:
                return out
            body, no = raw[ind:], self.no()
            if body.startswith('-[] '):
                self.i += 1
                out.append(split_cells(body[4:], no))
            elif body.startswith('-{} '):
                self.i += 1
                out.append(obj_from_cells(body[4:], no))
            elif body == '-':
                self.i += 1
                inner = self.items(ind + 1)
                if not inner:
                    raise LinError('a nested list marker with no items', no)
                out.append(inner)
            elif body.startswith('- '):
                self.i += 1
                out.append(self.value_after(body[2:], ind, no))
            elif body.startswith('+ '):
                self.i += 1
                out.append(self.obj(ind + 1, first=body[2:], first_no=no))
            elif body[:1] in '-+':
                raise LinError(f'bad list item {body!r}', no)
            else:
                return out


def read_header(line, no=1):
    f = line.split(' ')
    if len(f) < 2 or f[0] != 'LIN':
        raise LinError(f'the first line must be the header "LIN 1 [schema [url]]", found {line!r}', no)
    if f[1] != VERSION:
        raise LinError(f'unsupported LIN version {f[1]!r}', no)
    if len(f) > 4:
        raise LinError('too many fields in the header', no)
    kind = schema_id_kind(f[2]) if len(f) >= 3 else None
    if len(f) >= 3 and kind is None:
        raise LinError('the schema id must be a registered number (1 to 9 digits, no leading zero) '
                       'or a 39-digit provisional id', no)
    if len(f) == 4 and kind != 'provisional':
        raise LinError('a url may follow only a provisional schema id', no)
    if len(f) == 4 and (not f[3].startswith('https://') or len(f[3]) <= 8):
        raise LinError('the schema url must start with https://', no)
    return {'version': int(f[1]), 'schema': f[2] if len(f) > 2 else None, 'url': f[3] if len(f) > 3 else None}


def read_header_of(text):
    return read_header(text.split('\n', 1)[0])


def parse(text):
    """Parse without typing: returns (header, head, data). Bare number/boolean text is still _Bare,
    and each @ is still a reference; resolve() finishes the job."""
    if text.startswith('﻿'):
        raise LinError('a LIN file must not start with a byte-order mark', 1)
    if '\r' in text:
        raise LinError('LIN uses LF line ends, not CR LF', text[:text.index('\r')].count('\n') + 1)
    lines = text.split('\n')
    if lines and lines[-1] == '':
        lines.pop()
    if not lines:
        raise LinError('empty file')
    header = read_header(lines[0])
    try:
        sep = lines.index('---')
    except ValueError:
        sep = len(lines)
    head = _Reader(lines[1:sep], 2)
    first = head.peek()
    if first is None:
        doc = {}
    elif first == '[]':
        doc = []
        head.i += 1
    elif first.startswith('[] '):
        doc = split_cells(first[3:], head.no())
        head.i += 1
    elif first.startswith('['):
        if not first.endswith(']') or len(first) < 3:
            raise LinError(f'bad root table header {first!r}', head.no())
        no = head.no()
        cols = _columns(first[1:-1], no)
        head.i += 1
        doc = head.table_rows(cols, 1, no)
    elif first[:1] in '-+' and head.indent(first) == 0:
        doc = head.items(0)
    else:
        if head.indent(first):
            raise LinError('unexpected indentation', head.no())
        doc = head.obj(0)
    if head.peek() is not None:
        raise LinError(f'unexpected line {head.peek()!r}', head.no())
    data = {}
    r = _Reader(lines[sep + 1:], sep + 2)
    while (line := r.peek()) is not None:
        no = r.no()
        if not line.startswith('@'):
            raise LinError(f'a data line must start with @name, found {line!r}', no)
        r.i += 1
        sp = line.find(' ')
        name = parse_name(line[1:] if sp < 0 else line[1:sp], no)
        if name in data:
            raise LinError(f'duplicate data entry @{".".join(name)}', no)
        if sp < 0:
            items = r.items(1)
            if not items:
                raise LinError(f'data entry @{".".join(name)} has no value', no)
            data[name] = items
        else:
            data[name] = r.value_after(line[sp + 1:], 0, no)
    return header, doc, data


def _type_tree(tree, t, node):
    """Turn bare number/boolean text into values by the schema's types; refs keep their position."""
    if isinstance(tree, dict):
        return {k: _type_tree(x, t, t.prop(node, k)) for k, x in tree.items()}
    if isinstance(tree, list):
        return [_type_tree(x, t, t.item(node, i)) for i, x in enumerate(tree)]
    if isinstance(tree, _Bare):
        s = str(tree)
        if bare_reads_as_string(s, t.types(node)):
            return s
        return (s == 'true') if s in ('true', 'false') else parse_number(s)
    if isinstance(tree, _Ref):
        tree.value = node  # remember the position's schema node
        return tree
    if isinstance(tree, str):
        return str(tree)
    return tree


def resolve(header, head, data, schema=None):
    """Type the head, then put each data entry in place of its @, typed at that position."""
    t = _Typer(schema)
    doc = _type_tree(head, t, t.root())
    seen = set()

    def visit(c, k, v, n):
        if isinstance(v, _Ref):
            name = v.name or n
            if name not in data:
                raise LinError(f'no data entry for @{".".join(name)}')
            if name in seen:
                raise LinError(f'data entry @{".".join(name)} is referenced twice')
            c[k] = _type_tree(data[name], t, v.value)
            seen.add(name)

    walk_names(doc, visit)
    extra = [n for n in data if n not in seen]
    if extra:
        raise LinError('data entries with no @ in the head: ' + ', '.join('@' + '.'.join(n) for n in extra[:3]))
    return doc


class SchemaRequired(LinError):
    """The file names a schema that was not supplied."""


def loads(text, schema=None, *, ignore_schema=False, header_out=None, sha256=None):
    """Read LIN text into a JSON value.

    A file whose header names a schema is read with that schema (pass it as `schema`, as a JSON
    value). A provisional id is checked against the schema itself. A registered number is checked
    when you pass `sha256`, the full hash the registry's index records for it. Pass
    ignore_schema=True to read such a file by the schema-less rules instead (spec section 7).
    """
    header, head, data = parse(text)
    if header_out is not None:
        header_out.update(header)
    if header['schema'] and not ignore_schema:
        if schema is None:
            raise SchemaRequired(f'the file names schema {header["schema"]}; pass schema= or ignore_schema=True')
        from .jcs import schema_id, schema_sha256
        sid = header['schema']
        if schema_id_kind(sid) == 'provisional':
            got = schema_id(schema)
            if got != sid:
                raise LinError(f'schema mismatch: the header names {sid}, the schema given has id {got}')
        elif sha256 is not None and schema_sha256(schema) != sha256:
            raise LinError(f'schema mismatch: the schema given is not the one registered as {sid}')
    elif ignore_schema:
        schema = None
    return resolve(header, head, data, schema)
