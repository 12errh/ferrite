# SEMANTICS.md — the emission cookbook

**Status:** NORMATIVE. Every rule here is pinned by a golden test in `tests/golden/`.
**Audience:** anyone writing or reviewing `ferrite/codegen/emit.py`.
**Partially generated:** the examples in §3–§12 are extracted from `tests/golden/` by `tools/gen_semantics.py`. Edit the golden fixture, not this file, for anything in a `python`/`rust` example pair.

Read `TRD.md` §3 first. This document is the detail; the TRD is the model.

---

## 1. The three governing rules

**Rule 1 — Mutable Python containers are reference types, so they become `Rc<RefCell<…>>`.**
Cloning an `Rc` produces an alias. Python assignment produces an alias. They match by construction, so we never need alias analysis to be correct.

**Rule 2 — Every transpiled function returns `PyResult<T>`, every call site uses `?`.**
Uniform, boring, impossible to get subtly wrong.

**Rule 3 — Never hold a `RefCell` borrow across a call into transpiled user code.**
A double-borrow panics, and a panic crossing the PyO3 boundary can take down the host interpreter. Borrows *may* be held across `pyrt::` and `std::` calls, which never re-enter user code. Enforced by the lint in `codegen/lint.py` (T-036).

---

## 2. Type mapping

| Python | Rust | Value/Ref | `Copy`? |
|---|---|---|---|
| `int` | `i64` | value | yes |
| `float` | `f64` | value | yes |
| `bool` | `bool` | value | yes |
| `str` | `pyrt::PyStr` | value (immutable) | no — `Rc` clone |
| `None` | `()` | value | yes |
| `list[T]` | `pyrt::PyList<T>` | **reference** | no — `Rc` clone = alias |
| `dict[K,V]` | `pyrt::PyDict<K,V>` | **reference** | no — `Rc` clone = alias |
| `set[T]` | `pyrt::PySet<T>` | **reference** | no — `Rc` clone = alias |
| `tuple[A,B]` | `(A, B)` | value | if A, B are |
| `Optional[T]` | `Option<T>` | follows T | follows T |
| `@dataclass Foo` | `pyrt::PyObj<Foo>` | **reference** | no |
| `@dataclass(frozen=True) Foo` | `Foo` | value | no — derives `Clone` |

### 2.1 Name mangling

| Python name | Rust name |
|---|---|
| `snake_case` | unchanged |
| `Foo.bar` (method) | `Foo__bar` (free function, `self` as first param) |
| `Foo` (class) | `Foo` (struct) |
| collides with Rust keyword | `r#name` |
| collides with `self` `Self` `super` `crate` | `name_py` |
| leading underscore `_x` | unchanged |

Emit the mangling table into a comment at the top of the generated `lib.rs` so a human reading the output can map back.

---

## 3. Functions and calls

```python
def add(a: int, b: int) -> int:
    return a + b

def use() -> int:
    return add(1, 2)
```

```rust
pub fn add(a: i64, b: i64) -> pyrt::PyResult<i64> {
    return Ok(pyrt::add_i64(a, b)?);
}

pub fn use_py() -> pyrt::PyResult<i64> {
    return Ok(add(1, 2)?);
}
```

Notes:
- All parameters owned. No `&T`, no lifetimes, anywhere.
- `-> None` becomes `-> pyrt::PyResult<()>`, with a trailing `Ok(())`.
- Default arguments (R-9) emit an overload wrapper:
  ```rust
  pub fn f(a: i64) -> pyrt::PyResult<i64> { f__full(a, 10) }
  pub fn f__full(a: i64, b: i64) -> pyrt::PyResult<i64> { /* body */ }
  ```
- Keyword arguments at the call site are reordered to positional at lowering time. They never reach codegen.

---

## 4. Arithmetic

Every arithmetic operation routes through `pyrt`. **Never emit a raw Rust `+`, `/`, or `%` on `i64`.**

| Python | Rust | Raises |
|---|---|---|
| `a + b` (int) | `pyrt::add_i64(a, b)?` | `OverflowError` |
| `a - b` (int) | `pyrt::sub_i64(a, b)?` | `OverflowError` |
| `a * b` (int) | `pyrt::mul_i64(a, b)?` | `OverflowError` |
| `a / b` | `pyrt::truediv(a, b)?` → always `f64` | `ZeroDivisionError` |
| `a // b` (int) | `pyrt::floordiv(a, b)?` | `ZeroDivisionError` |
| `a % b` (int) | `pyrt::modulo(a, b)?` | `ZeroDivisionError` |
| `a ** b` | `pyrt::pow_i64(a, b)?` | `OverflowError`, `ValueError` on negative exp |
| `-a` (int) | `pyrt::neg_i64(a)?` | `OverflowError` at `i64::MIN` |
| float ops | native `+ - * /`, no checks | — |
| `a & b` etc. | native, `i64` only | — |

### 4.1 The floor-division trap

This is the highest-risk emission in the project.

| Expression | Python | Rust native `/` and `%` |
|---|---|---|
| `-7 // 2` | `-4` | `-3` ✗ |
| `-7 % 2` | `1` | `-1` ✗ |
| `7 // -2` | `-4` | `-3` ✗ |
| `7 % -2` | `-1` | `1` ✗ |

`pyrt::floordiv` and `pyrt::modulo` implement Python's semantics. Both have dedicated `cargo test` cases with every sign combination (T-011), and a conformance fixture `F002_arith` that fuzzes negative operands.

### 4.2 Integer overflow (PRD ID-2)

`int` is `i64`. Overflow raises `OverflowError` rather than promoting to a bignum. This is a documented deviation. It is **never** silent wraparound — every op is `checked_*`.

---

## 5. Comparison, boolean, truthiness

```python
if a < b and xs:
    ...
```

```rust
if (a < b) && xs.truthy() {
    ...
}
```

- Scalar comparison uses native Rust operators. Safe: UTF-8 byte ordering matches code-point ordering, so `PyStr` comparison is correct natively too.
- List/tuple comparison is elementwise-lexicographic: `pyrt::cmp_list(&a, &b)`.
- **Truthiness is not `bool`.** `pyrt::Truthy` is a trait implemented for every mapped type:
  `0`→false, `0.0`→false, `""`→false, empty container→false, `None`→false, everything else→true.
  Emit `.truthy()` wherever Python would coerce, never a bare value.
- `and`/`or` preserve short-circuiting via Rust `&&`/`||`. When operands are non-`bool` and the *value* is used (`x = a or b`), lower to a ternary instead.
- Chained comparison `a < b < c` lowers to `(a < b) && (b < c)` with `b` bound to a temporary first, so it is evaluated once.

```python
x = a or b        # value, not condition
```
```rust
let x: i64 = { let __t = a; if __t.truthy() { __t } else { b } };
```

---

## 6. Control flow

```python
while n > 1:
    if n % 2 == 0:
        n = n // 2
    else:
        n = 3 * n + 1
```

```rust
while n > 1 {
    if pyrt::modulo(n, 2)? == 0 {
        n = pyrt::floordiv(n, 2)?;
    } else {
        n = pyrt::add_i64(pyrt::mul_i64(3, n)?, 1)?;
    }
}
```

- `elif` lowers to nested `if`/`else` before codegen.
- `break`/`continue` map directly.
- `for i in range(n)` emits `for i in pyrt::range(0, n, 1)?`. `pyrt::range` handles negative steps and empty ranges; it returns `PyResult` because `step == 0` raises `ValueError`.

---

## 7. Containers

### 7.1 Construction and the `mut` surprise

```python
xs: list[int] = []
xs.append(1)
```

```rust
let xs: pyrt::PyList<i64> = pyrt::PyList::new();
xs.append(1)?;
```

`xs` is **not** `let mut`. Mutation goes through the `RefCell`, not through the binding. This is a direct and pleasant consequence of Rule 1: rebinding is the only thing that needs `mut`.

### 7.2 Indexing

```python
first = xs[0]
last  = xs[-1]
xs[0] = 99
```

```rust
let first: i64 = xs.getitem(0)?;
let last: i64 = xs.getitem(-1)?;
xs.setitem(0, 99)?;
```

Negative indices are handled inside `pyrt`. Out of range raises `IndexError`; it never panics.

### 7.3 Slicing

```python
mid = xs[1:-1]
rev = xs[::-1]
```

```rust
let mid: pyrt::PyList<i64> = xs.slice(Some(1), Some(-1), None)?;
let rev: pyrt::PyList<i64> = xs.slice(None, None, Some(-1))?;
```

Slices always produce a **new** container (a fresh `Rc`), matching Python.

### 7.4 Iteration — the snapshot rule

```python
for x in xs:
    total = total + x
```

```rust
for x in xs.iter_snapshot() {
    total = pyrt::add_i64(total, x)?;
}
```

`iter_snapshot()` clones the inner `Vec` and iterates the clone. This costs a copy, and it buys two things: no `RefCell` borrow is held across the loop body (Rule 3), and mutating the list during iteration cannot panic. It is a documented behaviour with its own conformance fixture (`F011_foreach`).

`for k in d:` iterates keys. `for k, v in d.items():` destructures a `(K, V)` tuple.

### 7.5 Dicts

```python
counts: dict[str, int] = {}
counts["a"] = 1
n = counts.get("b", 0)
m = counts["c"]          # KeyError
```

```rust
let counts: pyrt::PyDict<pyrt::PyStr, i64> = pyrt::PyDict::new();
counts.setitem(pyrt::PyStr::from("a"), 1)?;
let n: i64 = counts.get(&pyrt::PyStr::from("b"), 0);
let m: i64 = counts.getitem(&pyrt::PyStr::from("c"))?;
```

`PyDict` is backed by `IndexMap`, so iteration order is insertion order (PRD ID-4). Using `HashMap` would be a correctness bug, not a performance choice.

### 7.6 Membership

```python
if k in d: ...
if x in xs: ...
if c in s: ...
```
```rust
if d.contains(&k) { ... }
if xs.contains(&x) { ... }
if s.contains(&c) { ... }
```

---

## 8. Comprehensions

Desugared during lowering. Codegen never sees a comprehension node.

```python
ys = [x * 2 for x in xs if x > 0]
```

```rust
let ys: pyrt::PyList<i64> = pyrt::PyList::new();
for x in xs.clone().iter_snapshot() {
    if x > 0 {
        ys.append(pyrt::mul_i64(x, 2)?)?;
    }
}
```

Dict and set comprehensions follow the same shape with `setitem` / `add`. The loop variable is scoped to the desugared block and cannot leak (which incidentally matches Python 3 semantics).

---

## 9. Strings and f-strings

```python
name: str = "world"
greeting = f"hello {name}, {n} times"
parts = greeting.split(", ")
```

```rust
let name: pyrt::PyStr = pyrt::PyStr::from("world");
let greeting: pyrt::PyStr = pyrt::fstr(&[
    pyrt::PyStr::from("hello "),
    pyrt::to_str_any(&name)?,
    pyrt::PyStr::from(", "),
    pyrt::to_str_any(&n)?,
    pyrt::PyStr::from(" times"),
]);
let parts: pyrt::PyList<pyrt::PyStr> = greeting.split(&pyrt::PyStr::from(", "))?;
```

- `PyStr` wraps `Rc<String>`. Immutable, so clone is a refcount bump.
- `s[i]` emits `s.getitem(i)?` which is **O(n)** — it walks `chars()`. This is PRD ID-1. Codegen emits a warning when `s[i]` appears inside a loop. If the benchmark shows it is hot, T-061 adds a lazy `Vec<char>` cache.
- `to_str_any` is the transpiled `str()`, implemented via a `pyrt::ToPyStr` trait. Float formatting may differ from CPython in the last digit (PRD ID-3); the conformance harness compares floats with `math.isclose`.

---

## 10. Exceptions

### 10.1 Raise

```python
raise ValueError("bad input")
```
```rust
return Err(pyrt::PyErr::value_error("bad input"));
```

### 10.2 try / except

The IIFE lowering. This is the most intricate emission in the project.

```python
try:
    x = parse(s)
except ValueError:
    x = 0
```

```rust
let __t0: pyrt::PyResult<i64> = (|| {
    Ok(parse(s.clone())?)
})();
let x: i64 = match __t0 {
    Ok(__v) => __v,
    Err(__e) if __e.kind == pyrt::PyErrKind::ValueError => 0,
    Err(__e) => return Err(__e),
};
```

Why this works: the closure returns the values assigned inside the `try` body. Mutable containers are `Rc`, so the closure clones them freely and still aliases the originals. What would break is a *scalar* local that the body assigns and a handler does not, because the `Err` arm would then have no value to yield — that is exactly what **R-7** forbids (`FE011`).

If a handler does not assign such a local, it must be declared and initialised before the `try`, and the `Err` arm yields the pre-`try` value. Partial mutations performed by a body that then failed are therefore **not** observable for scalar locals. This is a consequence of the R-7 contract, not an undocumented divergence: the rule that forces pre-initialisation is what makes the simplification sound to state.

`__t0`, `__v`, `__e`, `__s` and `__t` are emitter temporaries. User identifiers starting with `__` are rejected (R-19) so these can never collide.

Multiple assignments in the body return a tuple:

```rust
let __t0: pyrt::PyResult<(i64, pyrt::PyStr)> = (|| { /* … */ Ok((a, b)) })();
let (a, b) = match __t0 { /* … */ };
```

Multiple handlers become additional guarded `Err` arms, in source order. The final `Err(__e) => return Err(__e)` arm is always emitted.

### 10.3 else and finally

```python
try:    body
except E: handler
else:   else_body
finally: cleanup
```

- `else_body` is appended to the `Ok` arm.
- `cleanup` is emitted as a block **after** the `match`, and additionally inlined before every `return Err(...)` inside the arms. Do not use a Rust `Drop` guard for this — it makes the control flow unreadable and interacts badly with the `?` operator.

### 10.4 assert

```python
assert n > 0, "must be positive"
```
```rust
if !(n > 0) {
    return Err(pyrt::PyErr::assertion_error("must be positive"));
}
```

Never emit Rust's `assert!` — it panics, and panics cross the PyO3 boundary.

---

## 11. Dataclasses

```python
@dataclass
class Point:
    x: float
    y: float

    def norm(self) -> float:
        return math.sqrt(self.x * self.x + self.y * self.y)

    def scale(self, k: float) -> None:
        self.x = self.x * k
        self.y = self.y * k
```

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Point { pub x: f64, pub y: f64 }

impl Point {
    pub fn new(x: f64, y: f64) -> pyrt::PyObj<Point> {
        pyrt::PyObj::new(Point { x, y })
    }
}

pub fn Point__norm(self_: pyrt::PyObj<Point>) -> pyrt::PyResult<f64> {
    let __s = self_.borrow();
    Ok(f64::sqrt(__s.x * __s.x + __s.y * __s.y))
}

pub fn Point__scale(self_: pyrt::PyObj<Point>, k: f64) -> pyrt::PyResult<()> {
    { let mut __s = self_.borrow_mut(); __s.x = __s.x * k; }
    { let mut __s = self_.borrow_mut(); __s.y = __s.y * k; }
    Ok(())
}
```

Notes:
- `Point(1.0, 2.0)` emits `Point::new(1.0, 2.0)`.
- `self` is `PyObj<Self>` **by value**, consistent with §3. Not `&self`, not `&mut self`.
- Each field write takes and releases its own `borrow_mut` in a scoped block. Verbose, but it makes Rule 3 mechanical — no borrow can span a call.
- `frozen=True` drops `PyObj` and emits a plain value struct. Field writes on a frozen dataclass are rejected at lowering (`FE211`).
- `field(default_factory=list)` emits a fresh container in `new`.

---

## 12. Builtins

| Python | Rust |
|---|---|
| `len(x)` | `x.len()` (returns `i64`) |
| `range(a, b, c)` | `pyrt::range(a, b, c)?` |
| `enumerate(x)` / `enumerate(x, n)` | `pyrt::enumerate(x, n)` |
| `zip(a, b)` | `pyrt::zip(a, b)` — truncates to shortest |
| `sum(x)` | `pyrt::sum_i64(&x)?` / `pyrt::sum_f64(&x)` |
| `min(x)` / `max(x)` | `pyrt::min(&x)?` — `ValueError` on empty |
| `sorted(x)` | `pyrt::sorted(&x)` — stable |
| `sorted(x, reverse=True)` | `pyrt::sorted_rev(&x)` |
| `abs(x)` | `pyrt::abs_i64(x)?` / `f64::abs(x)` |
| `any(x)` / `all(x)` | `pyrt::any(&x)` / `pyrt::all(&x)` |
| `reversed(x)` | `pyrt::reversed(&x)` |
| `int(x)` | `pyrt::to_int(x)?` — `ValueError` on bad string |
| `float(x)` | `pyrt::to_float(x)?` |
| `str(x)` | `pyrt::to_str_any(&x)?` |
| `bool(x)` | `x.truthy()` |
| `round(x)` / `round(x, n)` | `pyrt::round(x, n)` — banker's rounding |
| `divmod(a, b)` | `pyrt::divmod(a, b)?` |
| `print(...)` | `println!` via `pyrt::print_args` |
| `math.sqrt(x)` | `f64::sqrt(x)` |
| `math.floor(x)` | `pyrt::floor(x)` → `i64` |
| `math.pi` | `std::f64::consts::PI` |

The full allowed `math` surface is SUBSET.md §1.1; every member maps 1:1 to an `f64` method (`sin`, `cos`, `tan`, `log`, `log2`, `log10`, `exp`, `sqrt`, `hypot`, `isnan`, `isinf`, `isclose`) or an `std::f64::consts` constant (`PI`, `E`, `TAU`, `INFINITY`, `NAN`). `math.floor` / `math.ceil` are the two exceptions: they return `i64` via `pyrt::floor` / `pyrt::ceil`. Any `math` member outside §1.1 is rejected at the subset gate (`FE008`), never mapped approximately.

---

## 13. Cloning

Inserted by `ir/passes/clone_insert.py`, never by hand in the emitter.

The rule: when a non-`Copy` value is used and is still live afterwards, wrap the use in `Clone`. Liveness is computed with a straightforward backward pass over the statement list.

```python
def f(xs: list[int]) -> int:
    ys = xs          # alias, matches Python
    ys.append(1)
    return len(xs)   # 1 more than before
```

```rust
pub fn f(xs: pyrt::PyList<i64>) -> pyrt::PyResult<i64> {
    let ys: pyrt::PyList<i64> = xs.clone();   // Rc clone → alias
    ys.append(1)?;
    Ok(xs.len())                              // reflects the append
}
```

The generated code will contain visibly redundant clones. **Leave them.** The only thing permitted to remove them is `ir/passes/opt_escape.py` under `--opt`, and it must leave every conformance result byte-identical.

---

## 14. Deviation index

Every documented divergence from CPython, with its pinning test:

| ID | Deviation | Pinned by |
|---|---|---|
| ID-1 | `str` indexing is O(n) | `tests/conformance/F015_strings/` |
| ID-2 | `int` is 64-bit; overflow raises `OverflowError` | `tests/conformance/F002_arith/test_overflow.py` |
| ID-3 | float `repr` may differ in last digit | `tests/harness/test_float_compare.py` |
| ID-4 | dict iteration order preserved | `cargo test -p pyrt dict::order` |
| ID-5 | recursion depth is OS-stack bound | documented only |
| ID-6 | `for` iterates a snapshot of the container | `tests/conformance/F011_foreach/` |

**A divergence not on this list is a bug.** Adding one requires a PRD §6 edit and sign-off, not just a doc update here.
