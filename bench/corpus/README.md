# bench/corpus/

The frozen benchmark corpus for S1–S6 (PRD §5). 50 modules: 20 Advent-of-Code
solutions (2019–2023), 20 numeric/data-processing kernels, 10 extracted from real
OSS hot paths.

Each module is a directory: `mod.py`, `test_mod.py` (at least 5 tests including an
error case), and `bench.py` defining a representative workload.

**Freezing rule (TESTING.md §6.1):** once M3 starts, adding or removing a corpus
module requires a note in `docs/DECISIONS.md`. Otherwise S1 drifts upward for
reasons that have nothing to do with the transpiler getting better.

Empty until T-055. The measurement rules that make these numbers defensible are
in TESTING.md §6.
