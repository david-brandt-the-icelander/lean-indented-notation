"""Token counts for the benchmark sample (Appendix B of the spec).

    pip install tiktoken pyyaml
    python3 bench/fetch.py && python3 bench/run.py

Counts use OpenAI's o200k_base tokenizer as a stand-in for Claude's.
"""
import json
import os
import sys

import tiktoken
import yaml

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'python', 'src'))
import lin  # noqa: E402

enc = tiktoken.get_encoding('o200k_base')


def tokens(s):
    return len(enc.encode(s, disallowed_special=()))


def values(o):
    if isinstance(o, dict):
        for v in o.values():
            yield from values(v)
    elif isinstance(o, list):
        for v in o:
            yield from values(v)
    else:
        yield o if isinstance(o, str) else json.dumps(o)


def main():
    src = lin.loads(open(os.path.join(HERE, 'sources.lin'), encoding='utf-8').read())
    tot = {}
    smaller = n = 0
    for f in src['files']:
        p = os.path.join(HERE, 'files', f['file'])
        if not os.path.exists(p):
            continue
        raw = open(p, encoding='utf-8').read()
        d = lin.from_json(raw)
        mini = tokens(json.dumps(d, separators=(',', ':'), ensure_ascii=False))
        forms = {
            'JSON, 2-space indent': tokens(json.dumps(d, indent=2, ensure_ascii=False)),
            'JSON as found': tokens(raw),
            'YAML': tokens(yaml.safe_dump(d, sort_keys=False, allow_unicode=True, width=10**9)),
            'JSON, minified': mini,
            'LIN, plain form': tokens(lin.dumps(d)),
            'LIN, split form: head only': tokens(lin.dumps(d, split=True).split('\n---\n')[0]),
            'Values alone': tokens('\n'.join(values(d))),
        }
        for k, v in forms.items():
            tot[k] = tot.get(k, 0) + v
        smaller += forms['LIN, plain form'] < mini
        n += 1
    base = tot['JSON, 2-space indent']
    for k, v in tot.items():
        print(f'{k:30}{v:>10,} {v / base:7.1%}')
    print(f'{n} files; LIN is smaller than minified JSON in {smaller}')


if __name__ == '__main__':
    main()
