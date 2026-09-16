# TESTING.md — fixture conventions and benchmark methodology

**Companion to:** `PLAN.md` §A (the TDD doctrine)
**Purpose:** this file tells you *where files go and what shape they take*. `PLAN.md` tells you *when to write them*.

---

## 1. Layout

```
tests/
├── unit/                    fast, no subprocess, no cargo
│   ├── test_diagnostics.py
│   ├── test_subset.py       parametrised over every FE code
│   ├── test_infer.py
│   ├── test_lower.py
│   └── test_emit.py
├── golden/                  codegen determinism
│   └── F00N_<name>/
│       ├── input.py
│       ├── expected.rs
│       └── meta.toml        optional: flags, expected warnings
├── conformance/             behavioural equivalence — the specification
│   └── F0NN_<name>/
│       ├── mod.py
│       ├── test_mod.py
│       └── meta.toml        optional: milestone, xfail reason, tolerances
├── property/                differential fuzzing
│   ├── test_fuzz_corpus.py
│   ├── test_opt_equivalence.py
│   └── strategies.py
└── harness/                 tests OF the test machinery
    ├── fixtures/fib/        the hand-written reference crate (T-003)
    ├── test_swap.py
    ├── test_capture.py
    └── test_verify_e2e.py
```

`tests/harness/` is easy to skip and shouldn't be. It tests the thing that tests everything else. A silently broken harness produces green CI and a worthless product.

---

## 2. Writing a conformance fixture

This is the most common task in M1–M3. The shape is always the same.

**`tests/conformance/F021_try_basic/mod.py`** — valid PSS-0, exercising exactly one feature:

```python
def parse_or_default(s: str, default: int) -> int:
    try:
        n = int(s)
    except ValueError:
        n = default
    return n
```

**`tests/conformance/F021_try_basic/test_mod.py`** — plain pytest, no Ferrite imports:

```python
import mod   # resolved to .py or .so by the harness

def test_valid():          assert mod.parse_or_default("42", 0) == 42
def test_invalid():        assert mod.parse_or_default("abc", 7) == 7
def test_empty():          assert mod.parse_or_default("", -1) == -1
def test_negative():       assert mod.parse_or_default("-5", 0) == -5
def test_whitespace():     assert mod.parse_or_default("  12  ", 0) == 12
```

**Rules:**

1. **`test_mod.py` imports `mod`, never `ferrite`.** The harness swaps the implementation behind the import. If the test knows which implementation it's running, it isn't a conformance test.
2. **One feature per fixture.** `F021` tests `try`/`except`. It does not also test dicts. When it fails you want to know what broke.
3. **Cover the error path.** A feature's failure mode is where Python and Rust diverge most. If a fixture has no test that raises, it is incomplete.
4. **Cover negative numbers and empty containers.** These are where the mapping breaks: `//`, `%`, `min`, `max`, `[-1]`, `sum([])`.
5. **Name fixtures `F<nnn>_<snake_name>`** so `pytest -k F021` selects exactly one.

### meta.toml

```toml
milestone   = "M3"          # excluded from CI until M3 begins
float_tol   = 1e-9          # override math.isclose rel_tol
opt         = "both"        # run with and without --opt (default)
warns       = ["W001"]      # expected codegen warnings
```

**Never use `@pytest.mark.skip` or `xfail` on a conformance test.** Use `milestone` instead. Skipped tests are invisible; excluded milestones are counted and reported.

---

## 3. Writing a golden test

`input.py` is PSS-0. `expected.rs` is what codegen must produce, **after `rustfmt`**.

Write `expected.rs` **by hand, before implementing the emission.** You are designing the mapping. If the Rust you write by hand is awkward, the mapping is wrong — go back to `SEMANTICS.md` rather than building an awkward emitter.

**Updating goldens:**

```bash
uv run pytest tests/golden --snapshot-update
git diff tests/golden/
```

That `git diff` is your review of your own compiler change. Read every line. A golden diff you did not intend is a codegen regression, and it is much cheaper to catch here than in a conformance run three days later.

Golden tests are brittle by design. When one breaks, either you meant it (accept the diff) or you didn't (you just caught a bug).

---

## 4. Writing a property test

```python
from hypothesis import given
from tests.property.strategies import strategy_for

@given(xs=strategy_for("list[int]"), k=strategy_for("int"))
def test_scale_matches(xs, k):
    assert_same(mod_py.scale, mod_rs.scale, xs, k)
```

`assert_same` compares return value **and** exception type. A Python `ValueError` and a Rust `ValueError` must both occur, on the same input. An input where Python raises and Rust returns is the most dangerous class of bug this project can produce.

Default strategy bounds (TRD §7) keep integers under `2**40` so fuzzing exercises the algorithm rather than ID-2 overflow. Overflow gets its own targeted conformance test.

---

## 5. Float comparison

Never `==` on floats across implementations. The harness uses:

```python
math.isclose(a, b, rel_tol=1e-9, abs_tol=1e-12)
```

`nan` compares equal to `nan` for conformance purposes. `inf` must match exactly, including sign.

Nested floats inside containers are compared elementwise with the same rule. `ferrite/verify/compare.py` owns this logic; no test should implement its own.

---

## 6. Benchmark methodology

The S1–S6 numbers in `PRD.md` §5 are the release gate, so the measurement has to be defensible.

### 6.1 Corpus rules

`bench/corpus/` contains 50 modules, frozen at M3.

| Group | Count | Source |
|---|---|---|
| A | 20 | Advent of Code solutions, years 2019–2023 |
| B | 20 | numeric/data-processing kernels (statistics, string parsing, graph traversal, matrix ops) |
| C | 10 | functions extracted from real OSS projects' hot paths |

Each module ships with a pytest suite of **at least 5 tests** including one error case, plus a `bench.py` defining a representative workload.

**Freezing rule:** once M3 starts, adding or removing a corpus module requires a note in `DECISIONS.md`. Otherwise S1 drifts upward for reasons that have nothing to do with the transpiler getting better. This is the single easiest way to lie to yourself on this project.

### 6.2 Measuring S1–S3

```bash
uv run python bench/report.py --stage all
```

- **S1 (transpile rate):** modules producing Rust without `FE0xx`. Counted per module, not per function.
- **S2 (compile rate):** of S1, modules where `cargo check` is clean. Measured with `--repair` **off** by default; the `--repair` number is reported separately and labelled.
- **S3 (conformance rate):** of S2, modules where every test outcome matches. **Must be 100%.** A single divergence means the product's core claim is false.

### 6.3 Measuring S4 (speedup)

Getting this wrong is easy. The rules:

1. `cargo build --release`. A debug build is 10–50× slower and the number is meaningless.
2. 5 warmup iterations, discarded. Then 20 measured iterations.
3. Report the **median**, not the mean. One GC pause shouldn't move the number.
4. Same machine, same run, Python and Rust interleaved — not Python now and Rust after lunch.
5. Exclude the PyO3 call boundary from per-op reasoning: the workload calls the transpiled function once with a realistic input, not a million times with a trivial one.
6. Pin CPU frequency scaling if you can. Note it in the report if you can't.
7. Report the full distribution (min/median/p90/max) alongside the headline, so a reader can see the variance.

### 6.4 Regression gate

```bash
uv run python bench/report.py --check-regressions
```

Fails CI if S1, S2, or S3 drop at all, or if S4 drops by more than 10%. The 10% band absorbs machine noise. S1–S3 get no band because they are deterministic.

---

## 7. What to do when a conformance test fails

In order. Do not skip step 1.

1. **Is the Python right?** Run the suite with `FERRITE_IMPL=python`. If it fails there, the fixture is wrong, not the transpiler.
2. **Read the generated Rust.** It's in `ferrite_out/<module>/src/lib.rs`, formatted. This is usually enough.
3. **Check `SEMANTICS.md`** for the construct involved. Is the emission rule documented? Does the output match it?
4. **Is it in the deviation index** (`SEMANTICS.md` §14)? If yes and it's not pinned by a test, pin it.
5. **Minimise.** Cut the fixture down until it stops failing. The last thing you removed is the bug.
6. **Write the minimal failing case as a new fixture**, then fix.

Step 6 is not optional. `PLAN.md` §A.4 rule 4: every bug gets a failing test before it gets a fix. Including one-line typos, especially one-line typos.
