//! `pyrt` — the Ferrite runtime.
//!
//! This crate is the contract that every emission rule is written against, which
//! is why it is built before the codegen that targets it (TRD §4). Its public
//! surface is enumerated in `docs/TRD.md` §4 and consumed by `docs/SEMANTICS.md`.
//!
//! The governing model: Python's mutable containers are reference types and
//! Rust's are value types, so every mutable container maps to an `Rc<RefCell<..>>`
//! newtype here. Cloning one produces an alias, which is exactly what Python
//! assignment does (ADR-0002).
//!
//! # Invariants
//!
//! This crate contains **zero** `unwrap()`, zero `expect()`, and zero panics. A
//! panic crossing the PyO3 boundary aborts the host Python interpreter, so the
//! lints below are load-bearing safety rather than style. Every fallible
//! operation returns a `PyResult`, and indexing raises `IndexError` instead of
//! panicking (TRD §4).

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![forbid(unsafe_code)]
