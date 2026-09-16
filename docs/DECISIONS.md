# DECISIONS.md — architecture decision record

Every `[LOCKED]` decision in `TRD.md` has a record here explaining *why*, and what would make us change our mind.

**Purpose:** in week 7 someone (possibly you, possibly an agent) will look at `Rc<RefCell<>>` everywhere and think "this is obviously wrong, let me fix it." This file is the answer to that impulse. It records what was considered and rejected, so the same argument doesn't get re-litigated every month.

**Format:** one record per decision. Status is `ACCEPTED`, `SUPERSEDED by ADR-nnn`, or `PROPOSED`.

---

## ADR-0001 — Write the transpiler in Python

**Status:** ACCEPTED (2026-09-16)

**Context.** The natural instinct is to write a Rust-generating compiler in Rust, using `ruff_python_parser` or `rustpython-parser`.

**Decision.** The v0.1 transpiler is written in Python 3.12 using the stdlib `ast` module.

**Why.** Iteration speed is the binding constraint at MVP, not transpiler throughput. `ast` is free, correct, and requires zero dependency work. Nobody evaluating the product cares what the compiler is written in — they care whether their tests pass and how fast the output runs.

**Rejected alternative.** Rust frontend with `ruff_python_parser`. Faster, but roughly 2× the development time for a component whose runtime is irrelevant at this scale.

**Revisit when.** Transpile time exceeds 5 seconds on a typical module, or we want to ship the transpiler as a Rust binary with no Python dependency.

---

## ADR-0002 — Mutable containers map to `Rc<RefCell<T>>`

**Status:** ACCEPTED (2026-09-16)
**This is the most important decision in the project.**

**Context.** Python's `list`, `dict`, `set`, and class instances are reference types. `b = a; b.append(1)` mutates `a`. Rust's `Vec`, `HashMap`, and structs are value types with move semantics.

**Decision.** Every mutable Python container maps to an `Rc<RefCell<…>>` newtype in `pyrt`. `.clone()` produces an alias, exactly as Python assignment does. Immutable values (`int`, `float`, `bool`, `str`, `tuple`) map to plain Rust values.

**Why.** It is correct by construction. The alternative — mapping to `Vec<T>` and using static analysis to decide when aliasing occurs — requires a whole-program alias analysis that is both hard and unsound at the edges. An unsound alias analysis produces code that compiles, passes tests, and is silently wrong. That failure mode would destroy the product's only real claim.

The cost is real: `Rc` refcount bumps, `RefCell` borrow checks, and a snapshot copy per `for` loop. Measured against CPython, it is noise. The escape-analysis pass (T-060) removes most of it where it's provably safe.

**Rejected alternatives.**
- *`Vec<T>` + alias analysis.* Unsound at the edges, and the edges are where bugs live.
- *`Vec<T>` + reject all aliasing in PSS-0.* Rejects too much ordinary Python; S1 would collapse.
- *Arena allocation with indices.* Correct and fast, but the generated Rust becomes unreadable, which defeats the "you can read and audit the output" property.

**Revisit when.** S4 fails at ≥10× **after** `--opt` has landed, and profiling attributes the gap specifically to refcounting.

---

## ADR-0003 — Every function returns `PyResult<T>`

**Status:** ACCEPTED (2026-09-16)

**Context.** Most transpiled functions cannot fail. Returning `Result` from all of them is visibly redundant.

**Decision.** Uniform. Every transpiled function returns `pyrt::PyResult<T>`; every call site appends `?`.

**Why.** Analysing which functions are infallible requires a call-graph fixpoint (a function is infallible only if everything it calls is). That is an entire analysis pass whose only benefit is aesthetic — `Result<T, PyErr>` with a niche-optimised error costs close to nothing at runtime. Uniformity also eliminates a whole class of codegen bug where the emitter and the signature disagree about fallibility.

**Revisit when.** Never, for v0.x. If it matters at v1.0, it is a clean optimisation pass over a working system.

---

## ADR-0004 — `int` is `i64`, overflow raises

**Status:** ACCEPTED (2026-09-16)

**Context.** Python integers are arbitrary precision. Rust's are not.

**Decision.** `int` maps to `i64` with checked arithmetic on every operation. Overflow raises `OverflowError`. This is PRD deviation ID-2.

**Why.** Bignum-everywhere (`num-bigint`) costs roughly 20× on integer-heavy code, which would put S4 out of reach — and integer-heavy code is exactly our target workload. Checked arithmetic costs almost nothing because the branch is perfectly predicted.

The crucial property is that overflow is **loud**. Silent wraparound would be a correctness disaster; a raised `OverflowError` is a visible, testable, documented difference.

**Rejected alternative.** `i64` with a bignum fallback on overflow. Correct and reasonably fast, but it makes every integer a tagged union, which complicates every emission rule. Deferred to v0.2 behind `--bigint`.

**Revisit when.** A benchmark module genuinely needs values beyond `i64`, or a user asks for it.

---

## ADR-0005 — `try`/`except` lowers to an IIFE closure

**Status:** ACCEPTED (2026-09-16)

**Context.** Rust's `try` blocks are unstable. The `?` operator inside a labelled block still returns from the enclosing *function*, not the block. So there is no direct way to scope error propagation to a region.

**Decision.** A `try` body becomes an immediately-invoked closure returning `PyResult<(locals assigned in the body)>`. The `match` on its result implements the handlers. PSS-0 rule **R-7** requires every local the body assigns to be **bound on every path out of the whole `try` statement** — assigned in each `except` handler, or declared and initialised before the `try`.

The 2026-09-16 rewording of R-7 is load-bearing: the original text ("must be declared and initialised before the `try`") contradicted the canonical `try`/`except` example printed in TRD §3.5, SEMANTICS §10.2, and TESTING §2, all three of which bind the local inside the body and in the handler. Under the reworded rule those examples are legal and the closure still yields a value on both arms.

**Why.** It works on stable Rust today, and it composes with `?` naturally. It is only viable *because* of ADR-0002: mutable containers are `Rc`, so the closure clones them into itself and still aliases the originals. Only scalar locals are problematic, which is what R-7 covers.

**Rejected alternatives.**
- *Lowering each fallible call to an explicit `match` with a jump to the handler.* Needs `goto`, which Rust doesn't have.
- *Extracting each `try` body into a generated top-level function.* Requires passing an environment struct; more machinery, worse output readability.
- *Waiting for stable `try` blocks.* Not a plan.

**Revisit when.** `try` blocks stabilise in Rust.

---

## ADR-0006 — `for` iterates a snapshot

**Status:** ACCEPTED (2026-09-16)

**Context.** Iterating a `RefCell`-backed container while the loop body mutates it would either panic on double-borrow or require holding a borrow across user code, violating TRD Rule 3. A `RefCell` panic crosses the PyO3 boundary and can abort the host interpreter.

**Decision.** `for x in container:` clones the inner collection and iterates the clone. PRD deviation ID-6.

**Why.** It makes Rule 3 mechanical rather than analytical, and it removes an entire class of process-aborting failure. The cost is one copy per loop, which `--opt` removes when the container provably isn't mutated in the body.

**Behavioural note.** CPython raises `RuntimeError` when a dict changes size during iteration, and silently misbehaves for lists. Our snapshot semantics differ from both — deliberately, and documented. Pinned by `tests/conformance/F011_foreach/`.

**Revisit when.** Profiling shows snapshot copies are a top-three cost, and `--opt` cannot eliminate them.

---

## ADR-0007 — The LLM is a repair mechanic, not the engine

**Status:** ACCEPTED (2026-09-16)

**Context.** An LLM could plausibly translate Python to Rust directly, and would demo well.

**Decision.** The deterministic transpiler does all translation. The LLM is invoked only on `cargo check` failure, only behind `--repair`, capped at 3 iterations, off by default. Repaired modules are tagged in the report and must still pass conformance.

**Why.** Three reasons. Determinism: the same input must produce the same Rust, or golden tests are impossible and users can't trust the output. Honesty: `--repair` off by default means S1/S2 measure the real engine. Safety: an LLM will happily produce Rust that compiles and is subtly wrong, and the whole product is the guarantee against exactly that.

**Revisit when.** S1 plateaus below 60% and the gap is concentrated in a small number of constructs. Even then, the fix is more likely a new lowering rule than more LLM.

---

## ADR-0008 — pyright validates, we propagate

**Status:** ACCEPTED (2026-09-16)

**Context.** We need types for every expression. Full Hindley-Milner inference over Python is a research project.

**Decision.** PSS-0 requires annotations on every signature (R-1). `pyright --outputjson` gates the input. Our own forward dataflow pass propagates types to locals. No unification, no type variables, no backtracking.

**Why.** Annotations give us the seeds; propagation is ~400 lines. Full inference is six months and is the rock most Python-to-static-language projects have run aground on.

**Rejected alternative.** Runtime type tracing (MonkeyType) to infer annotations automatically. Attractive — it removes the annotation burden entirely — but it adds a "run your tests first" step and infers types that are merely *observed*, not guaranteed. Strong v0.2 candidate as an `--infer-from-tests` flag.

**Revisit when.** User feedback says the annotation requirement is the main adoption blocker. Track this explicitly; it is the most likely thing to be wrong about the MVP's shape.

---

## ADR-0009 — A root Cargo workspace, and `pyrt` does not enable `extension-module`

**Status:** ACCEPTED (2026-09-16)

**Context.** Every Rust command in `AGENTS.md`, `TRD.md` §11, and `PLAN.md` is workspace-scoped — `cargo build -p pyrt`, `cargo test -p pyrt`, `cargo clippy -p pyrt -- -D warnings`. The original TRD §9 layout specified only `pyrt/Cargo.toml`, so none of those commands could have run.

**Decision.** The repository root carries a `[workspace]` `Cargo.toml` with `members = ["pyrt"]`, `exclude = ["ferrite_out", "tests"]`, and `[workspace.dependencies]` pinning `pyo3` 0.22 and `indexmap` 2. `pyrt` does **not** enable `pyo3/extension-module`; generated crates do.

**Why.** Two separate reasons, both easy to undo by accident:
- The `exclude` list is load-bearing. A generated crate under `ferrite_out/` and the hand-written fixture crate under `tests/harness/fixtures/fib/fib_rs/` are standalone crates that must not be adopted into the workspace, or `cargo build -p pyrt` starts trying to build them.
- `pyo3/extension-module` suppresses linking libpython. `pyrt` must stay unit-testable on the Rust side with plain `cargo test -p pyrt` (TRD §4), so it must not enable it. The asymmetry with generated crates is deliberate.

**Rejected alternative.** `cargo build --manifest-path pyrt/Cargo.toml` everywhere. It works, but it silently invalidates every `-p pyrt` command already written into three documents.

**Revisit when.** The frontend is rewritten in Rust (ADR-0001) and the workspace gains a second real member.

---

## Adding a record

```markdown
## ADR-00NN — <one-line decision>

**Status:** PROPOSED | ACCEPTED (date) | SUPERSEDED by ADR-00MM

**Context.** What forced a choice.
**Decision.** What we do now. Present tense, concrete.
**Why.** The reasoning, including what it costs.
**Rejected alternatives.** Each with the reason it lost.
**Revisit when.** The specific observable condition that reopens this.
```

**"Revisit when" is mandatory.** A decision without a trigger to reconsider it becomes dogma, and dogma in a compiler is how you end up with an architecture nobody can defend but everybody is afraid to change.
