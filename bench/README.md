# Benchmark

Appendix B of the spec measures LIN on 124 permissively licensed JSON files from 87 GitHub projects.
`sources.lin` lists them by repository, branch, path, git blob hash and licence; the files themselves
are not redistributed.

```bash
pip install tiktoken pyyaml
python3 bench/fetch.py   # downloads into bench/files/ and checks each blob hash
python3 bench/run.py     # token totals for JSON, YAML, minified JSON and LIN
python3 bench/header.py  # the tokens each form of schema id adds to a header
```

For one file, `lin tokens FILE` gives the same counts.
