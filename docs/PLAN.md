# Implementation Plan — Ferrite MVP

**Companion to:** `PRD.md`, `TRD.md`
**Method:** strict test-driven development
**Duration:** 10 weeks, one full-time developer (or one developer + agents working task cards in parallel)

---

## Part A — TDD doctrine

### A.1 The loop, non-negotiable

Every task card below has a **RED** section and a **GREEN** section. You execute them in that order. Always.

```
RED    Write the test. Run it. Watch it fail.
       Confirm it fails for the RIGHT reason — read the error message.
       A test that fails with ImportError is not a red test, it is a broken test.

GREEN  Write the minimum code to pass. Not the elegant version. The minimum.
       Run the full suite, not just your test.

REFACT Clean up. Run the full suite again. Commit.
```

**If you cannot write the test first, the task is not specified well enough. Stop and specify it.** This applies double to AI agents: if a task card's RED section is ambiguous, raise it rather than inventing a test that happens to match whatever you were about to build.

### A.2 Test taxonomy

Four layers. Each answers a different question. Do not collapse them.

| Layer | Directory | Question it answers | Speed | Count at v0.1 |
|---|---|---|---|---|
| **Unit** | `tests/unit/` | Does this function do its job? | ms | ~250 |
| **Golden** | `tests/golden/` | Does codegen produce exactly this Rust? | ms | ~120 |
| **Conformance** | `tests/conformance/` | Does the Rust behave like the Python? | seconds | ~90 |
| **Property** | `tests/property/` | Does it behave like Python on inputs nobody thought of? | minutes | ~40 |

**Golden tests are brittle on purpose.** When you intentionally change codegen, `pytest --snapshot-update` regenerates them and you *read the diff*. That diff is your review of your own compiler change. This is a feature.

**Conformance tests are the specification.** If a behaviour is not covered by a conformance test, it is not specified and may silently regress.

### A.3 The feature recipe — memorise this

Adding any Python language feature follows the same seven steps. Every M1–M3 task card is an instance of this recipe.

```
1. SUBSET    Add the construct to docs/SUBSET.md. One sentence.
2. RED-CONF  tests/conformance/<Fnnn>_<name>/ → mod.py + test_mod.py
             Run: pytest -k Fnnn   → fails with FE001 UnsupportedConstruct
3. FRONTEND  Remove the construct from subset.py's reject list.
             Run: pytest -k Fnnn   → now fails in lower.py
4. IR        Add/extend the FIR node in ir/nodes.py. Extend lower.py.
             Run: pytest -k Fnnn   → now fails in emit.py
5. RED-GOLD  tests/golden/<Fnnn>_<name>/ → input.py + expected.rs
             Hand-write expected.rs FIRST. This is you designing the emission.
6. CODEGEN   Implement emission until golden passes.
             Run: pytest -k Fnnn   → conformance now passes too
7. PROPERTY  If the feature takes data (not just control flow), add a
             Hypothesis case in tests/property/.
```

If step 5 is hard to hand-write, your emission design is wrong. Go back to TRD §3 and pick a simpler mapping.

### A.4 Rules that prevent the classic failure modes

1. **Never commit with a skipped or xfail'd conformance test.** Use `@pytest.mark.milestone("M3")` and a collection filter instead, so unbuilt features are excluded rather than silently green.
2. **Never widen PSS-0 to make a benchmark pass.** Narrow the benchmark or add a task card.
3. **`pyrt` before codegen.** You cannot design an emission rule against an API that doesn't exist.
4. **Every bug gets a failing test before it gets a fix.** No exceptions, including one-line typos.
5. **Golden diffs are reviewed by a human**, even when an agent produced them.

---

## Part B — Milestones

| M | Weeks | Theme | Exit gate |
|---|---|---|---|
| M0 | 1 | Walking skeleton — harness only, no transpiler | `ferrite verify` works on hand-written Rust |
| M1 | 2–3 | Scalar core | 15 conformance tests green, no containers |
| M2 | 4–5 | Containers + `pyrt` | 45 conformance tests green |
| M3 | 6–7 | Errors + dataclasses | 90 conformance tests green, S1–S3 measurable |
| M4 | 8 | Escape analysis (`--opt`) | S4 ≥ 10× |
| M5 | 9 | Repair loop + diagnostics polish | S1 ≥ 60% with `--repair` |
| M6 | 10 | Benchmark, docs, release | All PRD §8 boxes ticked |

---

## M0 — Walking skeleton (week 1)

**The point of M0:** prove the *verification* mechanism works before building anything that generates code. If you cannot reliably run one pytest suite against two implementations and diff them, nothing downstream matters.

You write the Rust by hand this week. That is deliberate.

---
### T-001 — Repo scaffold
**Depends on:** —
**Files:** `pyproject.toml`, `ferrite/__init__.py`, `pyrt/Cargo.toml`, `.github/workflows/ci.yml`, `docs/`

**RED:** `tests/unit/test_smoke.py::test_imports` — `import ferrite; assert ferrite.__version__ == "0.1.0.dev0"`. Fails: `ModuleNotFoundError`.

**GREEN:** `uv init`, package layout per TRD §9, empty `pyrt` crate that compiles.

**Acceptance:**
```bash
uv run pytest tests/unit/test_smoke.py && cargo build -p pyrt
```

---
### T-002 — Diagnostics formatter
**Depends on:** T-001
**Files:** `ferrite/diagnostics.py`

**RED:** `tests/unit/test_diagnostics.py` — construct `FerriteError("FE001", Span("a.py", 42, 18, 16), note="...", help="...")`, assert `render()` output matches the exact block in PRD §F2, character for character.

**GREEN:** `Span` dataclass, `FerriteError`, source-line extraction, caret underline, `note`/`help` lines.

**Why this is task two:** every subsequent task raises errors. Build the error channel before you build anything that errors.

---
### T-003 — Hand-written reference crate
**Depends on:** T-001
**Files:** `tests/harness/fixtures/fib/` — `fib.py`, `test_fib.py`, `fib_rs/` (hand-written crate)

**RED:** none — this is a fixture, not code under test.

**GREEN:** `fib.py` with `fibonacci(n: int) -> int` and `sum_of_squares(xs: list[int]) -> int`. `test_fib.py` with 8 pytest cases including an error case. A hand-written Rust crate implementing both with PyO3, written the way you *wish* the transpiler would emit it.

**This artefact is your codegen design document.** Keep it in the repo forever. When you're unsure how to emit something in M1–M3, look here.

---
### T-004 — Import-swapping conftest
**Depends on:** T-003
**Files:** `ferrite/verify/swap.py`, `tests/harness/conftest.py`

**RED:** `tests/harness/test_swap.py::test_rust_impl_is_loaded` — with `FERRITE_IMPL=rust`, assert `fib.__file__.endswith(".so")`. Fails: still loads the `.py`.

**GREEN:** a `MetaPathFinder` installed before collection that redirects the target module name to the built `.so` when `FERRITE_IMPL=rust`.

**Trap:** pytest imports during collection. Install the finder in a root `conftest.py` at import time, not in a fixture.

---
### T-005 — Result capture hook
**Depends on:** T-004
**Files:** `ferrite/verify/capture.py`

**RED:** `tests/harness/test_capture.py` — running the fixture suite produces a JSON file with one record per test containing `nodeid`, `outcome`, `exception_type`, `exception_msg`, `duration`.

**GREEN:** a `pytest_runtest_makereport` hook writing `ferrite_results.json`.

---
### T-006 — `ferrite verify` end-to-end (M0 exit gate)
**Depends on:** T-003, T-004, T-005
**Files:** `ferrite/verify/harness.py`, `ferrite/cli.py`

**RED:** `tests/harness/test_verify_e2e.py`:
- `test_identical_impls_pass` — verify on the fixture exits 0
- `test_divergent_impl_fails` — deliberately break the hand-written Rust (return `n+1`), verify exits 1 and the report names the diverging test

**GREEN:** implement TRD §7 steps 1–6. `maturin build --release`, run both suites, diff, render.

> **M0 EXIT GATE:** `ferrite verify tests/harness/fixtures/fib` passes, and *detects a deliberately introduced bug*. Do not proceed to M1 until the second half is true. A harness that can't catch a bug is worse than no harness.

---

## M1 — Scalar core (weeks 2–3)

No containers. `int`, `float`, `bool`, `None` only. Everything returns `PyResult`.

---
### T-010 — `pyrt` error type
**Files:** `pyrt/src/err.rs`
**RED:** `cargo test -p pyrt err::` — construct each `PyErrKind`, assert `Display` matches CPython's format (`"ValueError: bad input"`).
**GREEN:** per TRD §3.5, plus `PyErr::value_error(msg)`-style constructors, plus `impl From<PyErr> for pyo3::PyErr`.

---
### T-011 — `pyrt` checked arithmetic
**Files:** `pyrt/src/int.rs`
**RED:** `cargo test -p pyrt int::` with these cases, written before any implementation:
```
floordiv(-7, 2)  == -4      // NOT -3
modulo(-7, 2)    ==  1      // NOT -1
floordiv(7, -2)  == -4
modulo(7, -2)    == -1
truediv(1, 3)    ≈  0.3333  // f64, always
truediv(1, 0)    -> ZeroDivisionError
add_i64(i64::MAX, 1) -> OverflowError
```
**GREEN:** implement. Cross-check every case against real CPython before trusting your Rust.

**This is the highest-risk task in M1.** Get it wrong and every numeric benchmark is silently off by one somewhere.

---
### T-012 — Type model + annotation parser
**Files:** `ferrite/types/model.py`
**RED:** `tests/unit/test_type_model.py` — parse `"list[dict[str, int]]"` → `TyList(TyDict(TyStr, TyInt))`; `"Optional[int]"` → `TyOpt(TyInt)`; `"Any"` → raises `FE050`.
**GREEN:** `Type` ADT + parser over `ast.expr` annotations.

---
### T-013 — Subset gate
**Files:** `ferrite/frontend/subset.py`
**RED:** `tests/unit/test_subset.py` — one test per PRD §4.2 exclusion, each asserting the correct FE code and span. Parametrise it:
```python
@pytest.mark.parametrize("src,code", [
    ("f = lambda x: x",        "FE001"),
    ("async def f(): pass",    "FE002"),
    ("def f():\n  yield 1",    "FE003"),
    ("class A(B): pass",       "FE004"),
    ("def f(*args): pass",     "FE005"),
    ("with open('x') as f: pass", "FE006"),
    ("import numpy",           "FE007"),
])
def test_rejected(src, code): ...
```
**GREEN:** an `ast.NodeVisitor` with an explicit allowlist. **Allowlist, not denylist** — anything unrecognised is rejected by default. This is the single most important defensive decision in the frontend.

---
### T-014 — pyright gate
**Files:** `ferrite/frontend/validate.py`
**RED:** a module with a genuine type error → `FE050` carrying pyright's message and span.
**GREEN:** subprocess `pyright --outputjson`, parse, map diagnostics to `FerriteError`.

---
### T-015 — Type inference, scalars
**Files:** `ferrite/types/infer.py`
**RED:** `tests/unit/test_infer.py` — annotated params seed the env; `x = a + b` infers `int`; rebinding `x` to a `str` raises `FE012`; using an undefined name raises `FE014`.
**GREEN:** TRD §6 forward pass, scalar types only.

---
### T-016 — FIR nodes + scalar lowering
**Files:** `ferrite/ir/nodes.py`, `ferrite/ir/lower.py`
**RED:** `tests/unit/test_lower.py` — lowering `x += 1` produces `Assign(Local('x'), BinOp('add', Name('x'), IntLit(1)))`; `a < b < c` produces a `BoolOp`; `elif` produces nested `If`.
**GREEN:** node definitions per TRD §5 plus desugaring for aug-assign, chained compare, elif.

---
### T-017 — Rust AST + emitter
**Files:** `ferrite/codegen/rust_ast.py`, `ferrite/codegen/emit.py`
**RED:** `tests/unit/test_emit.py` — build a `RsFn` by hand, assert the emitted text. Then `tests/golden/F001_arith/` with `input.py`/`expected.rs`.
**GREEN:** Rust AST dataclasses (`RsFn RsLet RsIf RsWhile RsCall RsBinary RsMatch RsBlock RsTry`) and a pretty printer. Pipe output through `rustfmt` so golden files are stable.

**Do not build a string-template emitter.** It works for two weeks and then you need parenthesisation rules and it collapses.

---
### T-018 — Crate scaffolding
**Files:** `ferrite/codegen/scaffold.py`
**RED:** `tests/unit/test_scaffold.py` — generated `Cargo.toml` pins exactly the crates in TRD §2 and nothing else; `pyproject.toml` has the right maturin config; `lib.rs` has `#[pymodule]` exporting each public function.
**GREEN:** template the three files.

---
### T-019 → T-025 — Scalar language features
Each one is an instance of the §A.3 recipe. Build in this order; each depends on the previous.

| ID | Feature | Conformance fixture | Key emission note |
|---|---|---|---|
| T-019 | `def` + `return` + calls | `F001_call` | every fn returns `PyResult`, every call gets `?` |
| T-020 | arithmetic + comparison | `F002_arith` | all ops route through `pyrt::`, never raw Rust operators |
| T-021 | `if`/`elif`/`else` | `F003_branch` | |
| T-022 | `while` + `break`/`continue` | `F004_while` | |
| T-023 | `for i in range(n)` | `F005_range` | `pyrt::range` iterator, handles negative step |
| T-024 | `bool` ops + truthiness | `F006_bool` | Python truthiness ≠ Rust `bool` — `pyrt::truthy()` |
| T-025 | `assert` | `F007_assert` | → `AssertionError`, not Rust `assert!` |

> **M1 EXIT GATE:** `ferrite build` + `ferrite verify` succeed on a recursive `fibonacci`, an iterative `collatz_length`, and a `newton_sqrt` using floats. 15 conformance tests green.

---

## M2 — Containers (weeks 4–5)

The `pyrt` container types land here. Write the Rust and its `cargo test` suite **before** touching codegen.

---
### T-030 — `PyStr`
**Files:** `pyrt/src/str.rs`
**RED:** `cargo test -p pyrt str::` — negative indexing; `IndexError` out of range (never a panic); slicing with negative and omitted bounds; `split`/`join` round-trip; multi-byte characters (`"héllo"[1] == "é"`).
**GREEN:** per TRD §4.

---
### T-031 — `PyList<T>`
**Files:** `pyrt/src/list.rs`
**RED:** `cargo test -p pyrt list::` — negative index; `IndexError`; slice assignment; `pop()` on empty → `IndexError`; **aliasing test**:
```rust
let a = PyList::from_vec(vec![1,2,3]);
let b = a.clone();
b.append(4)?;
assert_eq!(a.len(), 4);   // aliasing, exactly like Python
```
**GREEN:** per TRD §4. `iter_snapshot()` must clone the inner `Vec` (TRD §3.7).

---
### T-032 — `PyDict<K,V>`
**RED:** insertion order preserved across 100 inserts and a delete; missing key → `KeyError`; `get` with default; aliasing test as above.
**GREEN:** `IndexMap`-backed.

---
### T-033 — `PySet<T>` and T-034 — `builtins.rs`
**RED:** `sorted` stability; `sum` on empty list = `0`; `min`/`max` on empty → `ValueError`; `zip` truncates to shortest; `enumerate` with `start`.

---
### T-035 — Container types in inference and codegen
**RED:** golden test where `xs: list[int]` becomes `PyList<i64>` and every use site carries `.clone()`.
**GREEN:** extend `types/infer.py` and `codegen/emit.py`; implement `ir/passes/clone_insert.py` (liveness-driven).

---
### T-036 — Borrow-safety lint (critical)
**Files:** `ferrite/codegen/lint.py`
**RED:** `tests/unit/test_no_borrow_across_call.py` — construct an emission that would hold a `RefCell` borrow across a user-function call; assert it raises `FE300`.
**GREEN:** a static check over the Rust AST. A `RefCell` double-borrow panics, and a panic across the PyO3 boundary can take down the host interpreter. This lint is load-bearing safety, not tidiness.

---
### T-037 → T-043 — Container language features

| ID | Feature | Fixture |
|---|---|---|
| T-037 | list/dict/set literals, indexing, `len` | `F010_containers` |
| T-038 | `for x in <container>` (snapshot semantics) | `F011_foreach` |
| T-039 | slicing, incl. negative and step | `F012_slice` |
| T-040 | list/dict/set comprehensions | `F013_comprehension` |
| T-041 | container methods (TRD §4 list) | `F014_methods` |
| T-042 | `str` methods + f-strings | `F015_strings` |
| T-043 | tuples, unpacking, multiple assignment | `F016_tuple` |

**T-038 needs a dedicated conformance test for mutation during iteration** — the snapshot rule is a documented behaviour, so it must be pinned by a test.

> **M2 EXIT GATE:** 45 conformance tests green. `ferrite verify` passes on three real Advent-of-Code solutions from the corpus.

---

## M3 — Errors and dataclasses (weeks 6–7)

---
### T-050 — `raise` and `Result` propagation
**Fixture:** `F020_raise`. Exception kind and message must survive the PyO3 boundary — the conformance harness compares `type(exc).__name__`.

---
### T-051 — `try`/`except` (highest-complexity emission in the project)
**RED — write all four before implementing:**
1. `F021_try_basic` — catch, recover, continue
2. `F022_try_finally` — `finally` runs on both paths
3. `F023_try_rethrow` — uncaught kind propagates unchanged
4. `tests/unit/test_r7_violation.py` — assigning an uninitialised local inside `try` raises `FE011`

**GREEN:** TRD §3.5 IIFE lowering. Implement the R-7 check in `subset.py` *first*, so the closure design is sound before you write the emitter.

---
### T-052 — `@dataclass` → struct
**Fixture:** `F024_dataclass`. `PyObj<Foo>` = `Rc<RefCell<Foo>>`; field access goes through `.borrow()`; `__init__` becomes an associated `new`; `frozen=True` becomes a plain value type with `#[derive(Clone, PartialEq)]`.

---
### T-053 — methods on dataclasses
**Fixture:** `F025_methods`. `self` is `PyObj<Self>` by value, per TRD §3.3 (no `&mut self`).

---
### T-054 — Differential fuzzing (F4)
**Files:** `ferrite/verify/fuzz.py`
**RED:** `tests/property/test_fuzz_finds_bugs.py` — deliberately mis-emit `floordiv` as Rust `/`; assert the fuzzer finds a counterexample within 200 examples.
**GREEN:** TRD §7 strategy mapping.

**Same principle as T-006: prove the detector detects before you trust it.**

---
### T-055 — Benchmark corpus + report (F5)
**Files:** `bench/corpus/` (50 modules, each with tests), `bench/report.py`
**RED:** report emits the S1–S6 table; `--check-regressions` exits non-zero on a seeded regression.

> **M3 EXIT GATE:** 90 conformance tests green. First full benchmark run. Publish S1–S6 numbers even if they are bad — you now have a baseline to move.

---

## M4 — Optimisation (week 8)

### T-060 — Escape analysis
**Files:** `ferrite/ir/passes/opt_escape.py`
**RED:** `tests/property/test_opt_equivalence.py` — for every conformance fixture, results with `--opt` are byte-identical to results without. Plus golden tests showing a non-escaping `PyList<i64>` demoted to `Vec<i64>`.
**GREEN:** the four-condition analysis in TRD §3.6.

### T-061 — `PyStr` indexed fast path
Only if the benchmark says string indexing is hot. Add a lazily-built `Vec<char>` cache inside `PyStr`. **Do not build this speculatively.**

### T-062 — Perf regression gate
Wire S4 into CI with a 10% tolerance band.

> **M4 EXIT GATE:** S4 ≥ 10× median, with zero change to any conformance result.

---

## M5 — Repair loop and polish (week 9)

### T-070 — Structured `cargo` diagnostics
`cargo check --message-format=json`, parsed into `RustDiagnostic(code, span, message, suggestion)`, mapped back to the originating Python span via a source-map emitted during codegen.

**The source map is the hard part.** Every `RsNode` carries the `Span` of the FIR node it came from, which carries the Python span. Thread it through from T-017 onward — retrofitting it in week 9 is painful.

### T-071 — LLM repair (F6)
**RED:** `tests/unit/test_repair.py` with a recorded fixture — given a known `cargo` error and a known bad emission, assert the loop produces a compiling patch within 3 iterations. Use a stubbed LLM client; live calls never run in CI.
**GREEN:** prompt = Python source span + generated Rust + `cargo` diagnostics + TRD §3 mapping table. Response = unified diff. Apply, re-check, max 3 rounds. Tag `repaired: true` in the report.

**A repaired module must still pass conformance.** Repair fixes compile errors; it is never allowed to be the reason a behaviour test passes.

### T-072 — Diagnostics pass
Go through every FE code. Does the `help` line tell the user what to actually *do*? Rewrite the ones that don't. This is a half-day that determines whether anyone keeps using the tool after their first rejection.

---

## M6 — Release (week 10)

- **T-080** — `docs/SUBSET.md` synced to implementation, enforced by `tests/test_subset_doc_sync.py`
- **T-081** — `docs/SEMANTICS.md` auto-generated from conformance fixtures
- **T-082** — README quickstart, validated on a clean container in CI
- **T-083** — final benchmark run, publish report
- **T-084** — walk PRD §8 release gate, tick every box
- **T-085** — write the blog post. The S1–S6 table *is* the post.

---

## Part C — Working agreements

### C.1 Definition of done (every task card)

- [ ] RED test existed and failed for the stated reason before implementation
- [ ] Full suite green (`uv run pytest && cargo test -p pyrt`)
- [ ] `ruff check` + `mypy ferrite/` clean
- [ ] `cargo clippy -p pyrt -- -D warnings` clean
- [ ] Golden diffs, if any, reviewed by a human
- [ ] Docs updated if the task changed PSS-0 or an emission rule
- [ ] Commit message: `<task-id>: <what changed>`

### C.2 Branch and commit convention

```
branch:  t/<task-id>-<slug>          e.g. t/T-051-try-except
commit:  T-051: lower try/except via IIFE closure
```
One task per PR. A PR that touches two task cards gets split.

### C.3 Parallelisation

These tracks have no dependency on each other and can run concurrently:

- **Track R (Rust):** T-010, T-011, T-030 → T-034 — all of `pyrt`
- **Track F (frontend):** T-012 → T-016 — parse, subset, infer, lower
- **Track H (harness):** T-002 → T-006, T-054, T-055

Codegen (T-017+) depends on both R and F, so it is the merge point. If you are running multiple agents, give them Track R and Track F in week 2 and merge in week 3.

### C.4 Weekly checkpoint

Every Friday, answer in writing:

1. Which conformance tests went green this week?
2. What is the current S1/S2/S3 on the corpus? (from M3 onward)
3. Did any locked decision in the TRD turn out to be wrong? If yes, **edit the TRD first**, then the code.
4. Is PSS-0 still the same size, or did it quietly grow?

Question 4 is the one that kills projects like this. Scope creep in a transpiler looks like helpfulness and ends as an unfinishable compiler.
