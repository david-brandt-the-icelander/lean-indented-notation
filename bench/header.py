"""The token cost of the schema id in a LIN header (spec section 3, Appendix A).

    pip install tiktoken
    python3 bench/fetch.py && python3 bench/header.py

For each form of schema id it prints the tokens it adds to the header line, averaged over 500
random schemas, and what that would add to the Appendix B sample if every file named a schema.
"""
import base64
import hashlib
import os
import statistics
import sys

import tiktoken

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'python', 'src'))
import lin  # noqa: E402

random_digests = [hashlib.sha256(os.urandom(32)).digest() for _ in range(500)]
FORMS = {
    'old: sha256- and base64url (256 bits)': lambda d: 'sha256-' + base64.urlsafe_b64encode(d).decode().rstrip('='),
    'provisional: 128 bits, 39 decimal digits': lambda d: str(int.from_bytes(d[:16], 'big')).zfill(39),
    'registered number, 1 to 999': lambda d: str(1 + d[0] * 256 % 999),
    'registered number, 1,000 to 999,999': lambda d: str(1000 + int.from_bytes(d[:3], 'big') % 999000),
}


def main():
    encs = {n: tiktoken.get_encoding(n) for n in ('o200k_base', 'cl100k_base')}
    src = lin.loads(open(os.path.join(HERE, 'sources.lin'), encoding='utf-8').read())
    docs = []
    for f in src['files']:
        p = os.path.join(HERE, 'files', f['file'])
        if os.path.exists(p):
            docs.append(lin.from_json(open(p, encoding='utf-8').read()))
    print(f'{len(docs)} sample files')
    for name, enc in encs.items():
        tokens = lambda s: len(enc.encode(s, disallowed_special=()))  # noqa: E731
        sizes = [tokens(lin.dumps(d)) for d in docs]
        total, median = sum(sizes), statistics.median(sizes)
        print(f'\n{name}: sample {total:,} LIN tokens, median file {median:g}')
        print(f'{"schema id":42}{"tokens":>8}{"sample total":>14}{"median file":>13}')
        for form, f in FORMS.items():
            cost = statistics.mean(tokens('LIN 1 ' + f(d) + '\n') - tokens('LIN 1\n') for d in random_digests)
            print(f'{form:42}{cost:8.1f}{cost * len(docs) / total:14.2%}{cost / median:13.1%}')


if __name__ == '__main__':
    main()
