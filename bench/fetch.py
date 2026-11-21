"""Download the benchmark sample listed in sources.lin into bench/files/ (not committed).

    python3 bench/fetch.py

Each file is fetched from raw.githubusercontent.com at the recorded branch and checked against
its git blob hash; if the branch has moved on, the exact blob comes from the GitHub API instead.
"""
import base64
import hashlib
import json
import os
import sys
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, '..', 'python', 'src'))
import lin  # noqa: E402


def blob_sha(data):
    return hashlib.sha1(b'blob %d\0' % len(data) + data).hexdigest()


def get(url):
    req = urllib.request.Request(url, headers={'User-Agent': 'lin-bench'})
    with urllib.request.urlopen(req, timeout=30) as r:
        return r.read()


def main():
    src = lin.loads(open(os.path.join(HERE, 'sources.lin'), encoding='utf-8').read())
    out = os.path.join(HERE, 'files')
    os.makedirs(out, exist_ok=True)
    ok = 0
    for f in src['files']:
        dest = os.path.join(out, f['file'])
        if os.path.exists(dest) and blob_sha(open(dest, 'rb').read()) == f['blob']:
            ok += 1
            continue
        path = urllib.request.quote(f['path'])
        data = None
        try:
            data = get(f"https://raw.githubusercontent.com/{f['repo']}/{f['branch']}/{path}")
        except Exception as e:  # noqa: BLE001
            print('raw fetch failed', f['repo'], f['path'], e)
        if data is None or blob_sha(data) != f['blob']:
            try:
                j = json.loads(get(f"https://api.github.com/repos/{f['repo']}/git/blobs/{f['blob']}"))
                data = base64.b64decode(j['content'])
            except Exception as e:  # noqa: BLE001
                print('blob fetch failed', f['repo'], f['path'], e)
                continue
        with open(dest, 'wb') as fh:
            fh.write(data)
        ok += 1
    print(f'{ok} of {len(src["files"])} files in {out}')


if __name__ == '__main__':
    main()
