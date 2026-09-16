# PRD — Ferrite MVP

**Product:** Ferrite — a Python-to-Rust transpiler with a correctness guarantee
**Version:** 0.1 (MVP / proof of concept)
**Status:** Approved for build
**Owner:** _(you)_
**Last updated:** 2026-09-16

> Rename freely. `ferrite` is used throughout as the package name, CLI name, and Rust crate prefix. If you rename, do a single global find-replace before writing any code.

---

## 1. Problem

A Python developer with a slow function has three bad options:

1. Live with the slowness.
2. Spend 3–6 months learning Rust well enough to rewrite it safely.
3. Hire someone who already knows Rust.

Option 2 is the one most teams pick, and it is enormously expensive. The Rust learning curve (ownership, borrowing, lifetimes, `Result`) is front-loaded: you must understand all of it before you can write your first useful line. Meanwhile the developer already knows exactly what the function should do — they wrote it in Python.

**The gap is not knowledge of the algorithm. It is knowledge of Rust's memory model.**

## 2. Product thesis

Ferrite takes annotated Python, emits a Rust crate, and then **proves the Rust behaves identically** by running the developer's own test suite against it.

The guarantee is the product. A transpiler that emits plausible-looking Rust is worthless — no one will ship code they can't read and can't verify. A transpiler that emits ugly Rust and hands you a green test run is immediately shippable.

**Positioning statement:**

> Ferrite turns a slow Python function into a fast Rust extension module, without you writing Rust, and proves it still passes your tests.

### What this is NOT (v0.1)

- Not "never learn Rust again." When Ferrite fails, it fails loudly with a file and line number, and you fix the Python. It never emits silently-wrong Rust.
- Not a whole-program compiler. It transpiles one module. Your `main.py` keeps calling `numpy`, `requests`, `pandas`.
- Not an LLM wrapper. The deterministic core does the work; the LLM is an opt-in repair mechanic (§7).

## 3. Target user

**Primary persona — "Priya, backend engineer, 4 yrs Python, 0 yrs Rust."**

She owns a scoring service. One function, `score_batch()`, takes 400ms and it's the p99 bottleneck. She has good pytest coverage for it. She has read the Rust book twice and bounced off lifetimes both times.

**Her success moment:** she runs one command, gets a `.so` she can `import`, her existing tests pass unchanged, and the function now takes 12ms.

**Anti-persona:** someone who wants to port a Django app. Out of scope forever.

## 4. Scope

### 4.1 In scope — the input contract

Ferrite accepts **PSS-0** (Python Static Subset, level 0). Full grammar is normative in `docs/SUBSET.md`; summary:

| Category | Supported |
|---|---|
| Types | `int`, `float`, `bool`, `str`, `None`, `list[T]`, `dict[K,V]`, `set[T]`, `tuple[...]`, `Optional[T]`, `@dataclass` |
| Statements | assign, annotated assign, aug-assign, `if/elif/else`, `while`, `for..in`, `break`, `continue`, `return`, `pass`, `raise`, `try/except/else/finally`, `assert`, `def`, `@dataclass class` |
| Expressions | arithmetic, comparison, boolean, unary, subscript, slice, attribute, call, f-string, list/dict/set comprehension, ternary |
| Builtins | `len range enumerate zip min max sum sorted abs any all reversed int float str bool print isinstance` |
| Methods | list: `append extend insert pop remove sort index count clear`; dict: `get keys values items pop setdefault update clear`; set: `add discard remove update clear`; str: `split join strip lstrip rstrip upper lower startswith endswith replace find index count format` |
| Exceptions | `ValueError TypeError KeyError IndexError ZeroDivisionError OverflowError RuntimeError AssertionError StopIteration` |

**Hard requirement:** every function parameter and return value must carry a type annotation. No `Any`.

### 4.2 Explicitly out of scope for v0.1

`async`/`await` · generators and `yield` · `lambda` · decorators other than `@dataclass` · class inheritance · `*args`/`**kwargs` · `global`/`nonlocal` · `with` statements · closures that capture mutable state · `eval`/`exec`/`getattr`/`setattr`/`__getattr__` · metaclasses · multiple inheritance · walrus operator · `match` statements · third-party imports · file/network I/O · threading · arbitrary-precision integers (see §6, ID-2)

Encountering any of these produces a structured error, never a guess.

## 5. Success criteria

The MVP is done when **all six** hold on the frozen benchmark corpus (`bench/corpus/`, 50 Python modules — 20 Advent-of-Code solutions, 20 numeric/data-processing kernels, 10 extracted from real OSS hot paths):

| # | Metric | Target | Measured by |
|---|---|---|---|
| S1 | Transpile rate — modules producing Rust without `UnsupportedConstruct` | ≥ 60% | `ferrite bench --stage transpile` |
| S2 | Compile rate — of S1, modules where `cargo check` is clean | ≥ 95% | `ferrite bench --stage compile` |
| S3 | **Conformance rate** — of S2, modules where the original pytest suite passes against the Rust build | **100%** | `ferrite bench --stage conform` |
| S4 | Median speedup vs CPython 3.12 | ≥ 10× | `ferrite bench --stage perf` |
| S5 | False-success rate — Rust that compiles and passes tests but diverges under Hypothesis fuzzing | **0** | `pytest tests/property/` |
| S6 | Clippy-clean output (`-D warnings`, default lints) | ≥ 90% of S2 | `ferrite bench --stage lint` |

**S3 and S5 are non-negotiable.** They are the product. If S1 is only 45%, ship anyway and narrow the marketing. If S3 is 98%, do not ship — you have silently wrong output, which is worse than no product.

**Explicit non-goal:** idiomatic or beautiful Rust output. v0.1 output will contain redundant clones, `Rc<RefCell<>>` everywhere, and uniform `Result` returns. That is intended (see TRD §3).

## 6. Accepted semantic deviations

These are places where Ferrite's Rust deliberately differs from CPython. Each must be documented in the user-facing README and detected at transpile time where possible.

| ID | Deviation | Rationale | Mitigation |
|---|---|---|---|
| ID-1 | `str` indexing is O(n), not O(1) | Python indexes by code point, Rust by byte | Warn when `s[i]` appears inside a loop |
| ID-2 | `int` is 64-bit; overflow raises `OverflowError` instead of promoting to bignum | Bignum-everywhere costs ~20× perf | Checked arithmetic on every op; loud runtime error, never silent wraparound |
| ID-3 | `float` formatting may differ in last digit for `repr()` | Rust and CPython use different float→string algorithms | Conformance harness compares floats with `math.isclose`, not `==` |
| ID-4 | Dict iteration order preserved (matches Python 3.7+) | Uses `IndexMap`, not `HashMap` | None needed — this is correct behaviour, noted because `HashMap` would be wrong |
| ID-5 | Recursion depth limited by OS stack, not `sys.setrecursionlimit` | No cheap fix | Documented |

Any deviation discovered during development that is **not** on this list is a bug, not a feature. Add it here only after explicit sign-off.

## 7. Feature list

### F1 — `ferrite build <file.py>` (P0)
Transpiles one module to a Cargo crate in `./ferrite_out/<module>/`, builds it with maturin, emits a `.so`. Exits non-zero on any failure with a structured error.

### F2 — Structured rejection (P0)
Unsupported constructs produce:
```
error[FE001]: unsupported construct `Lambda`
  --> scoring.py:42:18
   |
42 |     ranked = sorted(items, key=lambda x: x.score)
   |                                ^^^^^^^^^^^^^^^^
   = note: PSS-0 does not support lambdas
   = help: define a module-level function and pass it by name
```
Every rejection carries: error code, file:line:col, source snippet, note, and an actionable `help`.

### F3 — Conformance harness (P0) — *this is the product*
`ferrite verify <file.py> --tests tests/test_scoring.py`
1. Runs the pytest suite against pure Python. Records results.
2. Builds the Rust module.
3. Monkeypatches the import so the same suite hits the Rust implementation.
4. Runs again. Diffs results, including exception types and messages.
5. Emits `ferrite_report.json` + a human-readable summary.

Exit 0 only if every test result is identical.

### F4 — Differential fuzzing (P0)
`ferrite fuzz <file.py>` — derives Hypothesis strategies from the type annotations, calls both implementations with the same inputs, asserts equal return values *and* equal exception types. Default 200 examples per function.

### F5 — Benchmark report (P1)
`ferrite bench` — runs the corpus, emits the S1–S6 table as markdown + JSON.

### F6 — Repair loop (P1, opt-in)
`--repair` flag. On `cargo check` failure, feeds the diagnostics + generated Rust + originating Python span to an LLM, applies the returned patch, re-checks. Max 3 iterations. **Off by default** so that S1/S2 measure the deterministic core honestly. Any repaired module is tagged `repaired: true` in the report and must still pass F3.

### F7 — Escape-analysis optimisation pass (P1)
Demotes `Rc<RefCell<T>>` containers to plain owned values where the container provably neither escapes the function nor is aliased. Gated behind `--opt`, must not change any F3 result.

## 8. Release gate checklist

Do not tag `v0.1.0` until every box is ticked:

- [ ] S1–S6 all green on the frozen corpus
- [ ] `docs/SUBSET.md` matches the implemented grammar exactly (verified by `tests/test_subset_doc_sync.py`)
- [ ] Every ID-* deviation in §6 has a dedicated conformance test proving the documented behaviour
- [ ] `ferrite build` on a file with an unsupported construct exits non-zero with an FE-code and never writes partial output
- [ ] README quickstart works on a clean machine in under 10 minutes
- [ ] Benchmark report published

## 9. Deferred to v0.2+

Generators → Rust iterators · classes with inheritance → traits · `numpy` array subset → `ndarray` · multi-file / package transpilation · VS Code extension · GitHub Action · borrow inference (`&T` params instead of owned) · bignum mode
