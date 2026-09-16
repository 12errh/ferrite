# ERRORS.md — diagnostic catalog

**Status:** NORMATIVE. Every error Ferrite emits is listed here.
**Enforced by:** `tests/test_error_catalog_sync.py` — every `FerriteError` code raised anywhere in `ferrite/` must appear in this file, and every code here must have at least one test that triggers it.

---

## 1. Why this file exists

Three failure modes it prevents:

1. **Code collisions.** Two modules independently picking `FE042`.
2. **Useless messages.** A code exists, is raised, and tells the user nothing actionable.
3. **Untested errors.** An error path that has never actually run, and segfaults the formatter when it finally does.

---

## 2. Reserved ranges

| Range | Module | Category |
|---|---|---|
| FE001–FE049 | `frontend/subset.py` | unsupported construct |
| FE050–FE099 | `frontend/validate.py` | annotation / pyright |
| FE100–FE149 | `types/infer.py` | inference failure |
| FE200–FE249 | `ir/lower.py` | lowering failure |
| FE300–FE349 | `codegen/` | emission failure |
| FE400–FE449 | `verify/` | build / conformance failure |

Never reuse a retired code. Mark it `RETIRED` and move on.

---

## 3. Rendering

All errors render through `ferrite/diagnostics.py::render()`:

```
error[FE001]: unsupported construct `Lambda`
  --> scoring.py:42:18
   |
42 |     ranked = sorted(items, key=lambda x: x.score)
   |                                ^^^^^^^^^^^^^^^^
   = note: PSS-0 does not support lambdas
   = help: define a module-level function and pass it by name
   = see:  docs/SUBSET.md R-6
```

**Every error must supply all four of:** `note` (what is wrong), `help` (what to do instead), `see` (the rule or doc section), and a `Span`. An error missing `help` fails `tests/test_error_catalog_sync.py`.

---

## 4. Catalog

### FE001–FE049 — unsupported construct

| Code | Message | Help |
|---|---|---|
| FE001 | unsupported construct `{node}` | rewrite using a supported construct; see SUBSET.md §3–§4 |
| FE002 | `async`/`await` is not supported | v0.1 transpiles synchronous code only |
| FE003 | generators and `yield` are not supported | return a `list` instead |
| FE004 | only `@dataclass` classes without inheritance are supported | remove the base class, or use composition |
| FE005 | `*args`/`**kwargs` are not supported | declare parameters explicitly |
| FE006 | `with` statement is not supported | restructure using `try`/`finally` |
| FE007 | import of `{module}` is not supported | only `dataclasses`, `typing`, `math` (SUBSET.md §1.1) |
| FE008 | `math.{name}` is not supported | see the supported list in SUBSET.md §1.1 |
| FE009 | `global`/`nonlocal` is not supported | pass state as a parameter and return it |
| FE010 | unhashable {dict key\|set element} type `{ty}` | keys must be `int`, `str`, `bool`, or a tuple of those (R-4) |
| FE011 | `{name}` is assigned inside `try` but not initialised before it | add `{name}: {ty} = <default>` before the `try` (R-7) |
| FE012 | `{name}` would change type from `{old}` to `{new}` | use a new variable name (R-2) |
| FE013 | `{name}` is defined inside a block and used after it | declare and initialise it before the block (R-3) |
| FE014 | cannot infer the type of this expression | add an annotation to the enclosing assignment |
| FE015 | `del` is not supported | use `.pop()` or `.clear()` |
| FE016 | `match` statement is not supported | use `if`/`elif` |
| FE017 | walrus operator `:=` is not supported | split into two statements |
| FE018 | `else` clause on a loop is not supported | use a flag variable |
| FE019 | format specifications are not supported | use plain `{}` and format the value beforehand |
| FE020 | comprehensions support one `for` and at most one `if` | use nested explicit loops (R-17) |
| FE021 | comprehension branches produce different types `{a}` and `{b}` | make both branches the same type |
| FE022 | builtin `{name}` is not supported | see SUBSET.md §5.1 |
| FE023 | method `{ty}.{name}` is not supported | see SUBSET.md §5.2 |
| FE024 | exception type `{name}` is not supported | use one of: ValueError, TypeError, KeyError, IndexError, ZeroDivisionError, OverflowError, RuntimeError, AssertionError, StopIteration |
| FE025 | nested functions and closures are not supported | move it to module level (R-6) |
| FE026 | nested destructuring in `for` is not supported | unpack inside the loop body (R-8) |
| FE027 | mutable default argument | use `None` as the default and build the value inside (R-9) |
| FE028 | `except` must name exactly one exception type | split into separate handlers (R-10) |
| FE029 | `{name}` is already defined at line {line} | rename one of them (R-12) |
| FE030 | module-level mutable state is not supported | annotate it as a constant, or pass it as a parameter (R-13) |
| FE031 | not all paths return a value | add a `return` to every branch, or annotate `-> None` (R-14) |
| FE032 | `is` may only be used with `None` | use `==` for value comparison (R-16) |
| FE033 | `{name}` shadows a builtin | choose a different name (R-18) |
| FE034 | decorator `@{name}` is not supported | only `@dataclass` is supported |
| FE035 | cannot assign to a field of a frozen dataclass | remove `frozen=True`, or construct a new instance |

### FE050–FE099 — annotation and type-checking

| Code | Message | Help |
|---|---|---|
| FE050 | missing type annotation on {parameter\|return} `{name}` | every parameter and return must be annotated (R-1) |
| FE051 | `Union[{a}, {b}]` is not supported | only `Optional[T]` (`T \| None`) is supported |
| FE052 | `Callable` is not supported | v0.1 has no first-class functions |
| FE053 | generics and `TypeVar` are not supported | use a concrete type |
| FE054 | bare `{name}` needs a type parameter | write `{name}[T]` |
| FE055 | string forward references are not supported | define the class before use |
| FE056 | abstract types (`Iterable`, `Sequence`, …) are not supported | use the concrete `list`/`dict`/`set` type |
| FE057 | `Any` is not supported | give the value a concrete type |
| FE060 | pyright reported an error: {message} | fix the type error in your Python before transpiling |

### FE100–FE149 — inference

| Code | Message | Help |
|---|---|---|
| FE100 | `{name}` is used before assignment | assign it before this point |
| FE101 | operands of `{op}` have incompatible types `{a}` and `{b}` | convert one explicitly with `int()` or `float()` |
| FE102 | ternary branches have types `{a}` and `{b}` | make both branches the same type |
| FE103 | `{name}` returns `{actual}` but is annotated `{declared}` | fix the annotation or the return |
| FE104 | container literal has mixed element types `{a}` and `{b}` | containers must be homogeneous |
| FE105 | cannot index `{ty}` | only `list`, `dict`, `str`, and `tuple` support `[]` |
| FE106 | `Optional[{ty}]` used without a `None` check | narrow with `if x is not None:` first |

### FE200–FE249 — lowering

| Code | Message | Help |
|---|---|---|
| FE200 | internal: no lowering rule for `{node}` | this is a Ferrite bug — please file it with the source snippet |
| FE201 | slice step of 0 | use a non-zero step |
| FE210 | assignment target `{form}` is not supported | assign to a name, subscript, or attribute |

### FE300–FE349 — emission

| Code | Message | Help |
|---|---|---|
| FE300 | internal: a `RefCell` borrow would span a call to `{fn}` | this is a Ferrite bug — the borrow-safety lint caught an unsafe emission |
| FE301 | internal: no emission rule for FIR node `{node}` | this is a Ferrite bug |
| FE302 | `rustfmt` failed on generated output | run with `--keep-unformatted` and inspect `ferrite_out/` |

### FE400–FE449 — build and verification

| Code | Message | Help |
|---|---|---|
| FE400 | `cargo check` failed with {n} error(s) | run with `--repair`, or inspect `ferrite_out/{module}/` |
| FE401 | `maturin build` failed | ensure Rust ≥ 1.75 and maturin ≥ 1.5 are installed |
| FE402 | conformance failure: {n} test(s) diverged | see `ferrite_report.json`; the Rust does not match the Python |
| FE403 | the Rust module aborted the process during `{test}` | a panic escaped `pyrt` — this is a Ferrite bug, please file it |
| FE404 | no test file supplied | `ferrite verify` requires `--tests`; without tests there is no guarantee |
| FE405 | fuzzing found a divergence in `{fn}` with input {input} | see `ferrite_report.json` for the counterexample |
| FE406 | repair loop exhausted after 3 attempts | the last `cargo` errors are in `ferrite_out/{module}/repair.log` |

---

## 5. Internal errors

`FE200`, `FE300`, `FE301`, `FE403` are **Ferrite bugs**, not user errors. They render differently:

```
internal error[FE300]: a RefCell borrow would span a call to `helper`
  --> scoring.py:17:5
   = note: this is a bug in Ferrite, not in your code
   = help: please file an issue with the snippet above
   = url:  https://github.com/<you>/ferrite/issues/new
```

Every internal error that fires in CI blocks the release. They are not acceptable failure modes.

---

## 6. Adding a code

1. Pick the next free number in the right range.
2. Add the row here, with a real `help` line. Write the `help` from the user's point of view: what do they *type* to fix this?
3. Add a test that triggers it (`tests/unit/test_subset.py` is parametrised, so usually one line).
4. Raise it via `FerriteError`, never with a bare `raise ValueError`.
