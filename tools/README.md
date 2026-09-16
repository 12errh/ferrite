# tools/

Repository tooling. Deliberately not part of the `ferrite` package — nothing here
ships.

- `gen_semantics.py` (T-081) — regenerates the `python`/`rust` example pairs in
  `docs/SEMANTICS.md` §3–§12 from `tests/golden/`. The golden fixture is the
  source of truth; a doc example that does not match a golden is a bug in the doc.
