# AGENTS.md — operating manual

> This file lives at the **repository root**, not in `docs/`. Claude Code, Cursor, and most agent harnesses look for it there. If you ever find it under `docs/`, it has been moved there by mistake — move it back.

You are working on **Ferrite**, a Python-to-Rust transpiler. Read this file completely before your first edit.

---

## Read order

1. This file
2. `docs/TRD.md` — architecture and locked decisions
3. `docs/PLAN.md` §A — the TDD doctrine
4. Your assigned task card in `docs/PLAN.md` Part B
5. `docs/SUBSET.md` — only if your task touches the accepted Python grammar
6. `docs/SEMANTICS.md` — only if your task touches emission
7. `docs/ERRORS.md` — if your task raises or adds a diagnostic code
8. `docs/DECISIONS.md` — **before** you conclude that a design choice is wrong

Do not read the whole codebase first. Read the task card, then the two or three files it names.

If two documents disagree, the narrower one wins: `SUBSET.md` over `PRD.md`, `ERRORS.md` over `PLAN.md`, `TRD.md` over everything except a `[LOCKED]`-tagged ADR in `DECISIONS.md`. Report the disagreement rather than silently picking one.

---

## The five rules

**1. Test first. Always.**
Your task card has a RED section. Write that test. Run it. Confirm it fails for the stated reason. Only then write implementation. If you have written implementation code before a failing test exists, delete it and start over.

**2. Locked decisions are locked.**
Anything marked `[LOCKED]` in the TRD is settled. If you believe one is wrong, **stop and say so** — write out the reasoning and wait. Do not implement an alternative and explain afterwards. A transpiler where the codegen disagrees with the spec is unfixable.

**3. Stay inside your task card.**
One task, one PR. If you notice a bug in adjacent code, write a failing test for it, add a task card, and move on. Do not fix it in this PR.

**4. Never widen PSS-0 to make something pass.**
If a benchmark module fails because it uses a lambda, the answer is `FE001`, not lambda support. Scope growth is the project's main failure mode.

**5. Reject by default.**
The subset gate is an allowlist. Any Python construct not explicitly allowed is rejected. When you add a node type to the visitor, the default branch must still raise.

---

## Commands

```bash
uv sync                                   # install (maturin + pyright come from the dev group)
uv run pytest                             # everything
uv run pytest tests/unit                  # fast, no subprocess, no cargo
uv run pytest tests/golden                # codegen determinism
uv run pytest tests/conformance -k F021   # one conformance fixture
uv run pytest --snapshot-update           # regenerate goldens (READ THE DIFF)
cargo build -p pyrt
cargo test -p pyrt                        # runtime unit tests
cargo clippy -p pyrt -- -D warnings
uv run ruff check . && uv run mypy ferrite/
uv run python bench/report.py             # S1–S6 table

uv run ferrite build   examples/collatz.py
uv run ferrite verify  examples/collatz.py --tests examples/test_collatz.py
uv run ferrite fuzz    examples/collatz.py
```

---

## Adding a language feature

This is the whole job for M1–M3. Follow it exactly, in order:

```
1. docs/SUBSET.md          one sentence describing the construct
2. tests/conformance/      mod.py + test_mod.py  → fails with FE001
3. ferrite/frontend/subset.py    allow the node  → fails in lower.py
4. ferrite/ir/nodes.py + lower.py                → fails in emit.py
5. tests/golden/           hand-write expected.rs FIRST
6. ferrite/codegen/emit.py                       → green
7. tests/property/         if the feature carries data
```

**Step 5 is the important one.** You are designing the emission by writing the Rust you want, by hand, before you write the code that generates it. If the hand-written Rust is awkward, the mapping is wrong — go back to TRD §3 rather than building an awkward emitter.

---

## Things that will bite you

| Trap | What happens | Rule |
|---|---|---|
| `RefCell` borrow held across a user call | double-borrow panic → aborts the host Python process | Never borrow across a call. T-036's lint enforces it. |
| Rust `/` and `%` on negatives | silently wrong answers for `//` and `%` | Always route through `pyrt::floordiv` / `pyrt::modulo` |
| `unwrap()` in `pyrt` | panic crosses the PyO3 boundary | `pyrt` denies `unwrap_used`, `expect_used`, `panic` at crate level |
| `HashMap` instead of `IndexMap` | dict iteration order diverges from Python | `PyDict` is `IndexMap`-backed, always |
| Mutating a list while iterating it | borrow panic, or divergence | `ForEach` always snapshots (TRD §3.7) |
| Forgetting `?` on a call | type error, or worse, a swallowed error | every `Call`/`MethodCall` with `fallible=True` emits `?` |
| Dropping the source span | week-9 repair loop can't map errors back | every `RsNode` carries the `Span` of its FIR node |
| A user identifier starting with `__` | collides with an emitter temporary (`__t0`, `__v`, `__e`, `__s`) | rejected by R-19 (`FE036`); never emit an unqualified temporary |
| Touching a field of a `frozen=True` dataclass | rejected at lowering, not silently `PyObj`-wrapped | `FE211` |
| String indexing in a loop | O(n²) | emit a perf warning; don't silently accept |

---

## Output discipline

**Generated Rust may be ugly.** Redundant `.clone()`, `Rc<RefCell<>>` on everything, uniform `Result` returns — all intended. Do not "improve" emission for aesthetics. The only thing permitted to remove clones is the escape-analysis pass (T-060), and it must leave every conformance result unchanged.

**Generated Rust may never be wrong.** If you are choosing between a fast emission you are unsure about and a slow one you are certain about, choose the slow one and add a task card.

---

## When you get stuck

1. Look at `tests/harness/fixtures/fib/fib_rs/` — the hand-written reference crate. It shows the emission style for the common cases.
2. Check `docs/SEMANTICS.md` §3–§12 — the emission cookbook, normative, with a worked example per construct.
3. Check `docs/TRD.md` §3 — the mapping table is normative.
4. Before "fixing" a design decision, read `docs/DECISIONS.md`. If your fix is not covered by a "Revisit when" trigger, propose an ADR instead of changing code.
5. If the task card's RED section is ambiguous, **ask**. Do not invent a test that happens to match what you were going to build. That is the failure mode where TDD stops working and nobody notices for three weeks.

---

## Reporting back

When you finish a task, report exactly this:

```
Task:        T-0NN
Red test:    <path::test_name>  — failed with: <the actual error>
Files:       <changed files>
Suite:       pytest <n> passed, cargo test <n> passed
Lint:        ruff clean, mypy clean, clippy clean
Goldens:     <none | n changed — diff summary>
Open:        <anything you noticed but did not fix, as proposed task cards>
```

If `Open` is empty every single time, you are not reading carefully enough.
