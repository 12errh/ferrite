# Ferrite

**Write Python. Ship Rust. Keep your tests.**

Ferrite transpiles a type-annotated Python module into a Rust extension module, then proves the Rust behaves identically by running *your* test suite against it.

> **Status: pre-alpha.** Not usable yet. See `docs/PLAN.md` for what exists and what doesn't.

---

## The idea

You have a slow Python function. You have tests for it. You don't have six months to learn Rust.

```bash
ferrite verify scoring.py --tests tests/test_scoring.py
```

```
✓ transpiled     scoring.py → ferrite_out/scoring/
✓ compiled       cargo check clean
✓ conformance    23/23 tests identical
✓ fuzzed         1,400 examples, 0 divergences

  score_batch    412ms → 11ms   (37× faster)

wrote scoring.so
```

Now `import scoring` gets the Rust one.

**The guarantee is the product.** A transpiler that emits plausible-looking Rust is worthless — nobody ships code they can't verify. A transpiler that emits *ugly* Rust and hands you a green test run is immediately useful.

---

## What it is not

- **Not "never learn Rust."** When Ferrite can't handle something, it says so, with a file, a line, and a suggested rewrite. It never guesses.
- **Not a whole-program compiler.** It transpiles one module. Your `main.py` keeps calling `numpy`, `requests`, and `pandas` exactly as before.
- **Not an LLM wrapper.** A deterministic compiler does the translation. An LLM is available as an opt-in repair mechanic for compile errors, and is off by default so the benchmark numbers mean something.

---

## Quickstart

**Requirements:** Python 3.12+, Rust 1.75+, [uv](https://docs.astral.sh/uv/), [maturin](https://www.maturin.rs/) 1.5+

```bash
git clone https://github.com/<you>/ferrite && cd ferrite
uv sync
cargo build -p pyrt
uv run pytest           # should be fully green

uv run ferrite build  examples/collatz.py
uv run ferrite verify examples/collatz.py --tests examples/test_collatz.py
uv run ferrite bench                        # the S1–S6 table
```

---

## What Python does it accept?

A subset called **PSS-0**, specified in `docs/SUBSET.md`. The short version:

**Yes:** annotated functions · `@dataclass` (no inheritance) · `int` `float` `bool` `str` `list` `dict` `set` `tuple` `Optional` · `if`/`while`/`for` · comprehensions · slicing · f-strings · `try`/`except`/`finally` · `math`

**No:** `async` · generators · `lambda` · inheritance · `*args`/`**kwargs` · closures · `eval`/`getattr` · third-party imports

Every parameter and return value must be annotated. Anything outside the subset is rejected with an error code and an actionable suggestion — never silently mistranslated.

---

## Documentation

| Doc | Read it when |
|---|---|
| **[AGENTS.md](AGENTS.md)** | you (or your agent) are about to write code — **start here** |
| [docs/PRD.md](docs/PRD.md) | you want to know what we're building and what "done" means |
| [docs/TRD.md](docs/TRD.md) | you need the architecture and the locked decisions |
| [docs/PLAN.md](docs/PLAN.md) | you need the TDD doctrine and the task cards |
| [docs/SUBSET.md](docs/SUBSET.md) | you need to know if a Python construct is accepted |
| [docs/SEMANTICS.md](docs/SEMANTICS.md) | you need to know what Rust a construct emits |
| [docs/ERRORS.md](docs/ERRORS.md) | you're adding or looking up a diagnostic code |
| [docs/TESTING.md](docs/TESTING.md) | you're writing a fixture or running the benchmark |
| [docs/DECISIONS.md](docs/DECISIONS.md) | you think a design choice looks wrong |

That last one matters. If you're about to "fix" `Rc<RefCell<>>` being everywhere, read ADR-0002 first.

---

## How it works

```
scoring.py → subset gate → pyright → type propagation → FIR
           → clone insertion → Rust AST → rustfmt → cargo check
           → maturin → your pytest suite, twice → diff → report
```

Two feedback loops: `cargo` errors go back to codegen, behavioural divergences go back to the IR. Details in `docs/TRD.md` §1.

**The core design bet:** Python's mutable containers are reference types, so they map to `Rc<RefCell<…>>`, not `Vec`. Cloning an `Rc` is an alias, which is exactly what Python assignment is. This makes aliasing correct by construction instead of by analysis. The output is uglier and the escape-analysis pass claws most of the cost back. See ADR-0002.

---

## Current status

| Metric | Target | Now |
|---|---|---|
| S1 transpile rate | ≥ 60% | — |
| S2 compile rate | ≥ 95% | — |
| S3 **conformance rate** | **100%** | — |
| S4 median speedup | ≥ 10× | — |
| S5 false successes | **0** | — |
| S6 clippy-clean output | ≥ 90% of S2 | — |

S3 and S5 are non-negotiable. If conformance is 98%, we don't ship — silently wrong output is worse than no product.

---

## Contributing

Read `AGENTS.md`. It applies to humans too.

Strict TDD: every task card has a RED section and a GREEN section, in that order. Every bug gets a failing test before it gets a fix, including one-line typos.

```
branch:  t/T-051-try-except
commit:  T-051: lower try/except via IIFE closure
```

One task card per PR.

---

## License

TBD.
