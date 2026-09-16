# TRD — Ferrite MVP

**Companion to:** `PRD.md`
**Audience:** implementers (human and AI agent)
**Status:** decisions are LOCKED. Deviating requires editing this document first.

---

## 0. How to read this

Every section marked **[LOCKED]** is a decision, not a suggestion. If you are an AI agent implementing a task and you believe a locked decision is wrong, stop and raise it — do not silently choose something else. Divergence between this document and the code is the single fastest way to kill this project.

Sections marked **[SPEC]** are normative behaviour. Your tests must encode them.

---

## 1. Architecture

```
scoring.py
    │
    ▼
┌─ frontend ──────────────────────────────────┐
│  parse.py     ast.parse → Python AST         │
│  subset.py    reject anything outside PSS-0  │
│  validate.py  run pyright, gate on errors    │
└──────────────────────────────────────────────┘
    │  (validated AST)
    ▼
┌─ types ──────────────────────────────────────┐
│  infer.py     forward dataflow, annotations  │
│               seed every local's type        │
└──────────────────────────────────────────────┘
    │  (AST + TypeEnv)
    ▼
┌─ ir ─────────────────────────────────────────┐
│  lower.py     AST → FIR (Ferrite IR)         │
│               desugars comprehensions,        │
│               try/except, aug-assign, slices  │
│  opt.py       escape analysis (--opt only)   │
└──────────────────────────────────────────────┘
    │  (FIR)
    ▼
┌─ codegen ────────────────────────────────────┐
│  rust_ast.py  build a Rust AST (not strings) │
│  emit.py      Rust AST → source text         │
│  scaffold.py  Cargo.toml, lib.rs, pyproject  │
└──────────────────────────────────────────────┘
    │  (crate on disk)
    ▼
┌─ verify ─────────────────────────────────────┐
│  cargo.py     cargo check --message-format   │
│  repair.py    LLM patch loop (--repair)      │
│  harness.py   maturin build → pytest diff    │
└──────────────────────────────────────────────┘
    │
    ▼  scoring.so  +  ferrite_report.json
```

## 2. Tech stack **[LOCKED]**

| Concern | Choice | Why this and not the alternative |
|---|---|---|
| Transpiler language | **Python 3.12** | `ast` is free and battle-tested; iteration speed matters more than transpiler speed at MVP. Rewriting the frontend in Rust (`ruff_python_parser`) is a v0.3 concern. |
| Parser | **stdlib `ast`** | LibCST preserves comments we don't need. Zero dependency. |
| Annotation validation | **pyright `--outputjson`** | Free, correct, maintained. We do not write a type checker; we write a type *propagator*. |
| Local type inference | **hand-written forward dataflow** | Full HM inference is a 6-month project. Annotations give us the seeds; propagation is ~400 lines. |
| IR | **Python `@dataclass` nodes, frozen** | Serialises to JSON for debugging and golden tests. |
| Codegen | **Rust AST dataclasses → pretty printer** | String templates become unmaintainable by week 3. Non-negotiable. |
| Formatting | **`rustfmt`** on output | Never hand-align generated code. |
| Rust runtime | **`pyrt` crate (we write it)** | The single highest-leverage component. See §4. |
| Python↔Rust bridge | **PyO3 0.22 + maturin** | Only serious option. |
| Test runner | **pytest** | |
| Property testing | **Hypothesis** | |
| CLI | **typer** | |
| Dep/venv management | **uv** | 10× faster than pip; matters when CI rebuilds 50 crates. |
| LLM (repair only) | **Claude via `anthropic` SDK** | Structured diagnostics in, unified diff out. |

**Pinned Rust crates for generated output:**
```toml
pyrt      = { path = "../../pyrt" }
pyo3      = { version = "0.22", features = ["extension-module"] }
indexmap  = "2"
```
No other crate may appear in generated `Cargo.toml` without editing this table.

---

## 3. The semantic model — the core of the product

### 3.1 Type mapping **[LOCKED] [SPEC]**

The governing rule:

> **Python's mutable containers are reference types. Rust's are value types. Therefore every mutable Python container maps to `Rc<RefCell<…>>`, and every immutable Python value maps to a plain Rust value.**

Cloning an `Rc` produces an alias, which is exactly what Python assignment does. This makes `b = a; b.append(1)` correct by construction instead of by analysis.

| Python | Rust | Mutable? | Notes |
|---|---|---|---|
| `int` | `i64` | no | checked arithmetic, see §3.4 |
| `float` | `f64` | no | |
| `bool` | `bool` | no | |
| `str` | `pyrt::PyStr` (`Rc<String>`) | no | O(n) indexing, ID-1 |
| `None` | `()` | — | |
| `list[T]` | `pyrt::PyList<T>` (`Rc<RefCell<Vec<T>>>`) | **yes** | |
| `dict[K,V]` | `pyrt::PyDict<K,V>` (`Rc<RefCell<IndexMap<K,V>>>`) | **yes** | K restricted, see §3.2 |
| `set[T]` | `pyrt::PySet<T>` (`Rc<RefCell<IndexSet<T>>>`) | **yes** | |
| `tuple[A,B]` | `(A, B)` | no | |
| `Optional[T]` | `Option<T>` | — | |
| `@dataclass Foo` | `pyrt::PyObj<Foo>` (`Rc<RefCell<Foo>>`) | **yes** | |
| `@dataclass(frozen=True) Foo` | `Foo` | no | value type, `#[derive(Clone)]` |

### 3.2 Hashability constraint **[SPEC]**

`f64` is not `Hash` in Rust, and Python floats-as-keys are a footgun anyway. PSS-0 restricts `dict` keys and `set` elements to: `int`, `str`, `bool`, and `tuple` composed only of those. Violation → `FE010: unhashable key type`.

### 3.3 Function signatures **[LOCKED] [SPEC]**

**Every** transpiled Python function becomes:

```rust
pub fn name(a: A, b: B) -> pyrt::PyResult<R>
```

where `PyResult<T> = Result<T, PyErr>`. Every call site appends `?`.

This is uniform and boring on purpose. Yes, some functions can't fail. No, we do not analyse that in v0.1 — `Result<T, PyErr>` with a niche-optimised error type costs close to nothing, and uniformity eliminates an entire class of codegen bug.

All parameters are taken **by value (owned)**. No `&T`, no lifetimes anywhere in generated code. `Rc` clones make this cheap for containers.

### 3.4 Arithmetic **[SPEC]**

| Python | Rust emission |
|---|---|
| `a + b` (int) | `pyrt::add_i64(a, b)?` — checked, raises `OverflowError` |
| `a / b` | `pyrt::truediv(a, b)?` — always `f64`, raises `ZeroDivisionError` |
| `a // b` | `pyrt::floordiv(a, b)?` — **floor** semantics, not Rust truncation |
| `a % b` | `pyrt::modulo(a, b)?` — Python sign semantics |
| `a ** b` | `pyrt::pow(a, b)?` |

`-7 // 2` is `-4` in Python and `-3` in Rust. `-7 % 2` is `1` in Python and `-1` in Rust. This is the single most likely source of a silent wrong answer in the whole project. It gets a dedicated conformance test with negative operands.

### 3.5 Errors **[SPEC]**

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PyErrKind {
    ValueError, TypeError, KeyError, IndexError,
    ZeroDivisionError, OverflowError, RuntimeError,
    AssertionError, StopIteration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PyErr { pub kind: PyErrKind, pub msg: String }
```

`raise ValueError("bad")` → `return Err(PyErr::value_error("bad"));`

**`try`/`except` lowering [LOCKED]:** immediately-invoked closure returning the locals it assigns.

```python
try:
    x = parse(s)
except ValueError:
    x = 0
```

```rust
let __t0: pyrt::PyResult<i64> = (|| { Ok(parse(s.clone())?) })();
let x: i64 = match __t0 {
    Ok(__v) => __v,
    Err(__e) if __e.kind == PyErrKind::ValueError => 0,
    Err(__e) => return Err(__e),
};
```

This works because mutable containers are `Rc` (cloned into the closure freely, still aliasing the original). It breaks only for scalar locals mutated inside `try`, which is why PSS-0 rule **R-7** requires: *any local assigned inside a `try` body must be declared and initialised before the `try` statement.* Enforced in `subset.py`, error `FE011`.

`finally` lowers to a block executed on both arms before the `match` result is used.

### 3.6 Ownership strategy **[LOCKED]**

**v0.1 = "Rc-everywhere, clone-liberally."**

- Containers and dataclass instances: `Rc<RefCell<T>>`. `.clone()` on every use — it's a refcount bump.
- Scalars and `str`: `.clone()` on every use after first. Trivial for `Copy` types, `Rc` bump for `PyStr`.
- The generated code will contain visibly redundant `.clone()` calls. **This is correct and intended.**

**The escape-analysis pass (F7, `--opt`) is the only thing permitted to remove them**, and only when a container provably (a) is created inside the function, (b) is never returned, (c) is never stored into another container, and (d) is never assigned to a second name. When all four hold, demote `PyList<T>` → `Vec<T>`. The pass must be a no-op on conformance results; `tests/property/test_opt_equivalence.py` asserts this.

### 3.7 Iteration **[SPEC]**

`for x in lst:` iterates over a **snapshot**: the generated code clones the `Vec` out of the `RefCell` before looping. This matches Python's behaviour closely enough for PSS-0 and — critically — prevents a `RefCell` double-borrow panic when the loop body mutates the list. A `RefCell` panic is an abort, not a `PyErr`, and an abort kills the host Python process. **Never hold a `RefCell` borrow across a call to user code.** This rule has a dedicated lint in `codegen/emit.py` and a test in `tests/test_no_borrow_across_call.py`.

---

## 4. The `pyrt` runtime crate **[SPEC]**

Located at `pyrt/`. This is ordinary hand-written Rust, fully unit-tested on the Rust side with `cargo test`. Public surface:

```
pyrt/src/
  lib.rs      re-exports; PyResult<T>
  err.rs      PyErr, PyErrKind, constructors, Display
  int.rs      add_i64 sub_i64 mul_i64 floordiv modulo truediv pow abs_i64
  str.rs      PyStr: new len getitem slice split join strip upper lower
              startswith endswith replace find count format_args
  list.rs     PyList<T>: new from_vec len getitem setitem slice append
              extend insert pop remove sort index count clear iter_snapshot
  dict.rs     PyDict<K,V>: new len getitem setitem get keys values items
              pop setdefault update contains clear
  set.rs      PySet<T>: new len add discard remove contains update clear
              union intersection difference
  obj.rs      PyObj<T>: Rc<RefCell<T>> newtype with borrow helpers
  builtins.rs range enumerate zip min max sum sorted any all reversed
              to_int to_float to_str to_bool
```

**Non-negotiable invariants:**

- Every indexing method accepts negative indices (`lst[-1]`) and raises `IndexError`, never panics.
- Every method that can fail returns `PyResult<T>`. **`pyrt` contains zero `unwrap()`, zero `expect()`, zero direct panics.** Enforced by `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` in `lib.rs`.
- `PyDict` uses `IndexMap` so insertion order is preserved (PRD ID-4).
- `PyStr::getitem(i)` uses `chars().nth()` — O(n), documented, correct.

Write `pyrt` **before** the codegen that targets it. Its API shape determines every emission rule.

---

## 5. FIR — the intermediate representation **[SPEC]**

Frozen dataclasses in `ferrite/ir/nodes.py`. Every node carries `span: Span` (file, line, col) for error reporting, and expression nodes carry `ty: Type`.

```
Module(functions: list[Func], structs: list[Struct])
Func(name, params: list[Param], ret: Type, body: list[Stmt], span)
Struct(name, fields: list[Field], frozen: bool, methods: list[Func])

Stmt  = Let(name, ty, init: Expr)           # first binding
      | Assign(target: Place, value: Expr)
      | ExprStmt(Expr)
      | If(cond, then: [Stmt], els: [Stmt])
      | While(cond, body: [Stmt])
      | ForEach(var, iter: Expr, body: [Stmt])   # iter is always snapshotted
      | Break | Continue
      | Return(Expr | None)
      | Raise(kind: PyErrKind, msg: Expr)
      | TryBlock(body, handlers: [Handler], els, finally_, bound: [str])
      | Assert(cond, msg: Expr | None)

Place = Local(name) | Index(base: Expr, idx: Expr) | Field(base: Expr, name)

Expr  = IntLit | FloatLit | StrLit | BoolLit | NoneLit
      | Name(name)
      | BinOp(op, lhs, rhs)          # op is a pyrt function, never raw Rust
      | UnaryOp(op, operand)
      | Compare(op, lhs, rhs)
      | BoolOp(op, operands)         # short-circuit preserved
      | Call(callee: str, args: [Expr], fallible: bool)
      | MethodCall(recv, method, args, fallible: bool)
      | Index(base, idx)
      | Slice(base, lo, hi, step)
      | FieldAccess(base, name)
      | ListLit | DictLit | SetLit | TupleLit
      | StructInit(name, fields)
      | Ternary(cond, then, els)
      | FStr(parts: [Expr | str])
      | Clone(inner)                 # inserted by the clone pass, never by lowering
```

**Desugared away during lowering — these never reach codegen:**
comprehensions (→ `Let` + `ForEach` + `append`) · aug-assign (`x += 1` → `Assign(x, BinOp)`) · chained comparison (`a < b < c` → `BoolOp`) · `elif` (→ nested `If`) · `for i in range(n)` (→ `ForEach` over a `pyrt::range` iterator) · default arguments (→ overload wrapper) · f-strings keep structure but interpolations become `to_str` calls.

**Two mandatory post-lowering passes, in this order:**
1. `ir/passes/clone_insert.py` — inserts `Clone` nodes wherever a non-`Copy` value is used and is still live afterwards. Uses a simple liveness analysis over the statement list.
2. `ir/passes/opt_escape.py` — the `--opt` demotion pass. Skipped by default.

---

## 6. Type inference **[SPEC]**

`ferrite/types/infer.py`, single forward pass per function:

1. Seed `TypeEnv` from parameter annotations and the return annotation.
2. Walk statements in order. `Let` computes the init expression's type and binds it.
3. `Assign` to an existing local **must** produce the same type. A local's type is fixed at first binding. Rebinding with a different type → `FE012: variable 'x' would change type from int to str`.
4. `If`/`While`/`ForEach` bodies inherit the env; locals first bound inside a branch are **not** visible after it (forces the R-7-style explicitness). → `FE013`.
5. Any expression whose type cannot be resolved → `FE014: cannot infer type` with the span.

There is no unification, no type variables, no backtracking. If you find yourself wanting them, you have left PSS-0 — narrow the subset instead of widening the inferencer.

---

## 7. Verification harness — the product **[SPEC]**

`ferrite/verify/harness.py`. Algorithm:

```
1. baseline = run_pytest(test_file, impl="python")
     capture per-test: outcome, return values via a conftest hook,
     exception type + message
2. crate = build(source)            # F1
3. so    = maturin_build(crate)
4. patched = run_pytest(test_file, impl="rust")
     sys.modules[module_name] is replaced by the compiled .so
     before collection
5. diff(baseline, patched):
     - outcome must match exactly
     - exception kind must match exactly
     - exception message: compared, mismatch = WARN not FAIL
     - float returns compared with math.isclose(rel_tol=1e-9)
     - all other returns compared with ==
6. exit 0 iff zero FAILs
```

**The `.so` must never be able to abort the process.** Any `RefCell` double-borrow or arithmetic panic is a harness-level FAIL, not a crash. Generated crates set `panic = "unwind"` and PyO3 catches unwinds at the boundary.

### Differential fuzzing (F4)

`ferrite/verify/fuzz.py` maps annotations to Hypothesis strategies:

| Annotation | Strategy |
|---|---|
| `int` | `st.integers(min_value=-2**40, max_value=2**40)` |
| `float` | `st.floats(allow_nan=False, allow_infinity=False)` |
| `str` | `st.text()` |
| `list[T]` | `st.lists(strategy(T), max_size=50)` |
| `dict[K,V]` | `st.dictionaries(strategy(K), strategy(V), max_size=30)` |
| `Optional[T]` | `st.none() \| strategy(T)` |

The int range is deliberately below `2**63` so we fuzz the algorithm, not ID-2 overflow. A separate targeted test covers overflow behaviour.

---

## 8. Error codes

Reserve ranges so codes never collide across modules.

| Range | Module | Meaning |
|---|---|---|
| FE001–FE049 | `frontend/subset.py` | unsupported construct |
| FE050–FE099 | `frontend/validate.py` | pyright / annotation errors |
| FE100–FE149 | `types/infer.py` | inference failures |
| FE200–FE249 | `ir/lower.py` | lowering failures |
| FE300–FE349 | `codegen/` | emission failures |
| FE400–FE449 | `verify/` | build / conformance failures |

Every error is raised as `FerriteError(code, span, note, help)` and rendered by a single formatter in `ferrite/diagnostics.py`. No `print()` or bare `raise ValueError` anywhere in the codebase. Enforced by `tests/test_no_bare_errors.py`.

---

## 9. Repository layout

```
ferrite/
├── AGENTS.md                  ← read this first if you are an AI agent
├── README.md
├── pyproject.toml             uv-managed
├── docs/
│   ├── PRD.md  TRD.md  PLAN.md
│   ├── SUBSET.md              ← normative PSS-0 grammar
│   └── SEMANTICS.md           ← Python→Rust mapping, generated from tests
├── ferrite/
│   ├── cli.py  diagnostics.py
│   ├── frontend/  parse.py  subset.py  validate.py
│   ├── types/     model.py  infer.py
│   ├── ir/        nodes.py  lower.py  passes/
│   ├── codegen/   rust_ast.py  emit.py  mangle.py  scaffold.py
│   └── verify/    cargo.py  harness.py  fuzz.py  repair.py
├── pyrt/                      ← Rust runtime crate
│   ├── Cargo.toml
│   └── src/  lib.rs err.rs int.rs str.rs list.rs dict.rs set.rs obj.rs builtins.rs
├── tests/
│   ├── golden/                <name>/input.py + expected.rs
│   ├── conformance/           <name>/mod.py + test_mod.py
│   ├── property/
│   └── harness/
├── bench/
│   ├── corpus/                50 frozen modules
│   └── report.py
└── .github/workflows/ci.yml
```

---

## 10. Performance budget

To hit S4 (10× median speedup) with the Rc-everywhere model:

| Cost | Budget | Mitigation if exceeded |
|---|---|---|
| `Rc` clone per container use | ~2ns | acceptable; `--opt` removes most |
| `RefCell` borrow check | ~1ns | acceptable |
| `PyO3` call boundary (per call) | ~200ns | amortised — we transpile the *whole* hot function, not per-op |
| Checked arithmetic | ~0 (branch predicted) | acceptable |
| `PyStr` indexing | **O(n)** | the one real risk — warn on `s[i]` in a loop |

If S4 fails, the fix is `--opt` (F7) and a `Vec<char>` fast path for indexed strings — in that order. Do not start micro-optimising codegen before the benchmark tells you where the time actually goes.

---

## 11. CI gates

`.github/workflows/ci.yml` must run, and block merge on:

1. `uv run pytest tests/golden` — codegen determinism
2. `uv run pytest tests/conformance` — behavioural equivalence
3. `uv run pytest tests/property` — differential fuzzing
4. `cargo test -p pyrt` — runtime unit tests
5. `cargo clippy -p pyrt -- -D warnings`
6. `uv run ruff check . && uv run mypy ferrite/` — the transpiler itself is typed
7. `uv run python bench/report.py --check-regressions` — S1–S4 must not drop
