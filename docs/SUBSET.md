# SUBSET.md — PSS-0 (Python Static Subset, level 0)

**Status:** NORMATIVE. This document defines exactly what `ferrite build` accepts.
**Enforced by:** `ferrite/frontend/subset.py`
**Sync test:** `tests/test_subset_doc_sync.py` parses this file's rule IDs and asserts each has a corresponding check in `subset.py`. A rule here without an implementation fails CI, and vice versa.

---

## 0. Governing principle

`subset.py` is an **allowlist**. Every Python AST node type not named in §2–§6 is rejected with `FE001`. When you add a node to the visitor, the default branch must still reject.

This means PSS-0 can only grow deliberately. That is the point.

---

## 1. Module structure

A PSS-0 module is a single `.py` file containing, in any order:

- allowed imports (§1.1)
- module-level constants (R-13)
- `@dataclass` class definitions
- module-level function definitions

Nothing else at module level. No top-level statements, no `if __name__ == "__main__"`, no side effects at import time.

### 1.1 Allowed imports

Exactly these, and nothing else:

```python
from dataclasses import dataclass, field
from typing import Optional
import math
```

Any other import → `FE007`.

The `math` subset that transpiles: `sqrt floor ceil fabs pow log log2 log10 exp sin cos tan atan atan2 hypot pi e inf tau isnan isinf isclose`. Each maps 1:1 to an `f64` method or an `std::f64::consts` constant (SEMANTICS §12). Anything else on `math` → `FE008`.

---

## 2. Types

### 2.1 Allowed type expressions

| Form | Example |
|---|---|
| scalar | `int` `float` `bool` `str` `None` |
| list | `list[T]` |
| dict | `dict[K, V]` |
| set | `set[T]` |
| tuple | `tuple[A, B]`, `tuple[A, ...]` |
| optional | `Optional[T]` |
| dataclass | any `@dataclass` defined in this module |

Nesting is unrestricted: `dict[str, list[tuple[int, float]]]` is valid.

### 2.2 Rejected type expressions

`Any` (`FE057`) · `Union[A, B]` where neither is `None` (`FE051`) · `Callable` (`FE052`) · `TypeVar` / generics (`FE053`) · unparameterised `list` / `dict` / `set` (`FE054`) · string forward references (`FE055`) · `Iterator` / `Iterable` / `Sequence` (`FE056`)

`Optional[T]` and `T | None` are both accepted and mean the same thing.

---

## 3. Statements

| Allowed | Notes |
|---|---|
| `x: T = expr` | annotated assignment |
| `x = expr` | plain assignment, type inferred |
| `a, b = expr` | tuple unpacking, targets must be simple names (R-8) |
| `x[i] = expr` | subscript assignment |
| `obj.field = expr` | attribute assignment, non-frozen dataclass only |
| `x += expr` | and `-= *= /= //= %= **=` |
| `if` / `elif` / `else` | |
| `while` | with optional `break` / `continue` |
| `for x in expr:` | see R-8 |
| `return expr` / `return` | see R-14 |
| `raise E(msg)` | `E` from the list in §6 |
| `try` / `except` / `else` / `finally` | see R-7, R-10 |
| `assert cond` / `assert cond, msg` | |
| `pass` | |
| `def` | module level or dataclass method only (R-6) |
| `@dataclass class` | R-5 |
| expression statement | only if the expression is a call |

**Rejected:** `with` (`FE006`) · `async`/`await` (`FE002`) · `yield` (`FE003`) · `global`/`nonlocal` (`FE009`) · `del` (`FE015`) · `match` (`FE016`) · `lambda` (`FE001`) · walrus `:=` (`FE017`) · `for`/`while` with `else` clause (`FE018`) · bare `class` without `@dataclass` (`FE004`)

---

## 4. Expressions

| Category | Allowed |
|---|---|
| Literals | int, float, string, bytes ✗, bool, `None` |
| f-strings | yes, with `{expr}` and `{expr!r}`; **no format specs** (`{x:.2f}` → `FE019`) |
| Names | local, parameter, module constant, function name |
| Arithmetic | `+ - * / // % **` and unary `- +` |
| Bitwise | `& \| ^ ~ << >>` on `int` only |
| Comparison | `< <= > >= == !=`, chained (`a < b < c`) |
| Membership | `in`, `not in` |
| Identity | `is None`, `is not None` **only** (R-16) |
| Boolean | `and` `or` `not`, short-circuiting preserved |
| Ternary | `a if cond else b`, both branches must have the same type |
| Subscript | `x[i]`, negative indices supported |
| Slice | `x[a:b]`, `x[a:b:c]`, any bound omittable |
| Attribute | `obj.field`, `math.pi` |
| Call | function, dataclass constructor, allowed method, allowed builtin |
| Containers | `[...]` `{...}` `{k: v}` `(...)` literals |
| Comprehensions | list, dict, set — single `for`, optional single `if` (R-17) |

**Rejected:** generator expressions (`FE003`) · starred expressions (`FE005`) · nested comprehension `for` clauses (`FE020`) · a ternary inside a comprehension whose branches differ in type (`FE021`)

---

## 5. Builtins and methods

### 5.1 Builtins

```
len range enumerate zip min max sum sorted abs any all reversed
int float str bool round divmod print isinstance
```

Constraints:
- `sorted(x)` and `sorted(x, reverse=True)` only. `key=` requires a lambda, which is rejected (`FE001`).
- `min`/`max` accept either one iterable or two-plus scalars, not a mix.
- `isinstance` is allowed only against a single concrete type, and only to narrow `Optional[T]`.
- `print` is allowed but emits to stdout via `println!`; it is not captured by the conformance harness.
- `round(x)` and `round(x, n)` use **banker's rounding**, matching CPython.

**Rejected builtins:** `eval exec getattr setattr hasattr type id hash open input map filter iter next dir vars globals locals` → `FE022`

### 5.2 Methods

**list:** `append extend insert pop remove sort index count clear copy reverse`
**dict:** `get keys values items pop setdefault update clear copy`
**set:** `add discard remove update clear copy union intersection difference issubset issuperset`
**str:** `split rsplit join strip lstrip rstrip upper lower title capitalize startswith endswith replace find rfind index count isdigit isalpha isalnum isspace zfill ljust rjust format removeprefix removesuffix`

`str.format` is limited to positional `{}` and `{0}` placeholders with no format specification. Anything else → `FE019`.

Any method not on these lists → `FE023`.

---

## 6. Exceptions

Raisable and catchable:

```
ValueError TypeError KeyError IndexError ZeroDivisionError
OverflowError RuntimeError AssertionError StopIteration
```

Anything else, including user-defined exception classes → `FE024`.

---

## 7. Restrictions

These are the rules that are not expressible as "which AST nodes are allowed." Each has an ID that appears in error messages.

| ID | Rule | Error | Why |
|---|---|---|---|
| **R-1** | Every function parameter and return value carries a type annotation. `Any` is not a usable type (`FE057`, §2.2). | FE050 | We propagate types, we don't infer them. |
| **R-2** | A local's type is fixed at first binding. Rebinding with a different type is an error. | FE012 | Rust locals are monomorphic. |
| **R-3** | A local first bound inside an `if`/`while`/`for` body is not visible after that block. | FE013 | Avoids uninitialised-variable emission. |
| **R-4** | `dict` keys and `set` elements must be `int`, `str`, `bool`, or a `tuple` of those. | FE010 | `f64` is not `Hash` in Rust. |
| **R-5** | Classes must be decorated `@dataclass` and must not inherit. | FE004 | No trait/vtable machinery in v0.1. |
| **R-6** | Functions are module-level or dataclass methods. No nested functions, no closures. | FE025 | Closures require capture analysis. |
| **R-7** | Every local the `try` body assigns must be **bound on every path out of the whole `try` statement**: assigned in each `except` handler, or declared and initialised before the `try` (or both). | FE011 | The IIFE lowering (SEMANTICS §10) must yield a value on both the `Ok` and the `Err` arm. |
| **R-8** | A `for` target is a simple name, or a flat tuple of simple names. No nested destructuring. | FE026 | |
| **R-9** | Default argument values must be immutable literals (`int float str bool None` or tuples of those). | FE027 | Python's mutable-default trap; also simplifies emission. |
| **R-10** | An `except` handler names exactly one exception type and binds at most one name. No bare `except:`, no tuples of types. | FE028 | |
| **R-11** | Only the imports in §1.1. | FE007 | |
| **R-12** | Function and class names are unique within the module. No redefinition. | FE029 | |
| **R-13** | Module-level bindings must be annotated constants with literal initialisers. They are emitted as Rust `const`. No module-level mutable state. | FE030 | Global mutable state needs `static mut` or `OnceLock`; out of scope. |
| **R-14** | Every path through a function must `return` a value, unless the return annotation is `None`. | FE031 | |
| **R-15** | Recursion is allowed; mutual recursion is allowed. Recursion depth is bounded by the OS stack (PRD ID-5). | — | Documented, not enforced. |
| **R-16** | `is` and `is not` may only be used with `None`. | FE032 | Python identity has no Rust equivalent for value types. |
| **R-17** | Comprehensions have exactly one `for` clause and at most one `if` clause. | FE020 | |
| **R-18** | A function may not shadow a builtin name or a module constant. | FE033 | |
| **R-19** | Identifiers beginning with `__` (two underscores) are reserved for the emitter and are rejected in user code. | FE036 | The IIFE and dataclass lowerings introduce temporaries (`__t0`, `__v`, `__e`, `__s`); a user variable with the same name would collide silently. |

---

## 8. Quick reference — rejected constructs

| Construct | Code |
|---|---|
| `lambda` | FE001 |
| `async` / `await` | FE002 |
| `yield`, generator expression | FE003 |
| class inheritance, non-dataclass class | FE004 |
| `*args`, `**kwargs`, starred expr | FE005 |
| `with` statement | FE006 |
| disallowed import | FE007 |
| disallowed `math` member | FE008 |
| `global` / `nonlocal` | FE009 |
| unhashable dict key / set element | FE010 |
| uninitialised local assigned in `try` | FE011 |
| local assigned in `try` but not bound on every path out of it | FE011 |
| local changes type | FE012 |
| local escapes its block | FE013 |
| type cannot be inferred | FE014 |
| `del` | FE015 |
| `match` | FE016 |
| walrus `:=` | FE017 |
| `for`/`while` with `else` | FE018 |
| f-string or `.format` with format spec | FE019 |
| multi-`for` comprehension | FE020 |
| comprehension branch type mismatch | FE021 |
| rejected builtin | FE022 |
| unsupported method | FE023 |
| unsupported exception type | FE024 |
| nested function / closure | FE025 |
| nested destructuring in `for` | FE026 |
| mutable default argument | FE027 |
| bare or multi-type `except` | FE028 |
| duplicate definition | FE029 |
| module-level mutable state | FE030 |
| missing return on some path | FE031 |
| `is` with non-`None` | FE032 |
| shadowed builtin | FE033 |
| decorator other than `@dataclass` | FE034 |
| identifier starting with `__` | FE036 |
| `Any` in an annotation | FE057 |
| pyright reports a type error | FE060 |
| assignment to a field of a frozen dataclass (detected at lowering, SEMANTICS §11) | FE211 |

---

## 9. Growing the subset

PSS-0 grows only by task card. To add a construct:

1. Add it here with a rule ID or a table row.
2. Add the conformance fixture (`docs/TESTING.md` §2).
3. Remove the rejection from `subset.py`.
4. Implement lowering and emission.
5. Document the emission in `docs/SEMANTICS.md`.

Step 1 first, always. If you cannot write the one-line rule, you do not yet know what you are accepting.
