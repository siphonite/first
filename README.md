# FIRST

> FIRST is a deterministic crash and recovery testing framework for storage engines and WAL-based systems.

## The Problem

Modern databases and storage engines rely on strong crash-consistency guarantees.

They write data using WALs, manifests, page files, and metadata updates, then recover state after crashes by replaying logs or rebuilding in-memory structures. These recovery paths are some of the most critical — and most fragile — code in the system.

However, crash behavior is rarely tested systematically.

Across the ecosystem today:

- Unit tests validate logic assuming uninterrupted execution
- Integration tests exercise happy paths
- Benchmarks measure performance, not correctness
- Fault injection exists, but is often coarse or non-deterministic
- Crash consistency and power-loss behavior are tested ad-hoc, per project
- Storage correctness is largely enforced by tribal knowledge and past incidents

Real failures do not happen at function boundaries.

They happen:
- Between writes
- During system calls
- After data reaches the page cache but before persistence
- After a WAL append but before a commit marker
- After a rename but before directory fsync

These failure modes are difficult to reason about and easy to miss.

As a result, many storage systems ship with latent crash-consistency bugs that:
- Pass unit tests
- Pass integration tests
- Pass benchmarks
- Surface only under real crashes, often years later

Every major database eventually builds internal crash-testing and chaos infrastructure to address this.

But these solutions are:
- Reinvented independently
- Tightly coupled to internal codebases
- Hard to maintain
- Rarely exhaustive
- Not reusable across projects

There is no standard framework, shared language, or common methodology for testing crash correctness in storage systems.

FIRST exists to fill this gap.

## Why FIRST

Crash consistency failures are difficult to test because crashes are not normal execution paths.

Most testing frameworks treat crashes as rare, external events. When failures are injected, they are often:
- Random
- Non-deterministic
- Coarse-grained
- Difficult to reproduce

This makes failures hard to debug and discourages exhaustive testing.

FIRST takes a different approach.

FIRST treats crashes as a first-class part of execution.

Instead of asking:
> “Does this code work?”

FIRST asks:
> “At every possible crash point, does recovery preserve the system’s invariants?”

To answer this, FIRST provides:

- **Deterministic crash injection**  
  Crashes are injected at precise, replayable points, making failures reproducible.

- **Systematic exploration**  
  Crash points are enumerated deliberately, not discovered by chance.

- **Recovery-driven testing**  
  Every crash is followed by a restart and execution of recovery logic.

- **Invariant-based validation**  
  Tests assert properties that must always hold, rather than expected outputs.

By making crashes deterministic, observable, and repeatable, FIRST turns crash consistency from an ad-hoc practice into a testable property.

This allows storage systems to reason about correctness under failure with the same rigor applied to normal execution.

## What FIRST Guarantees

FIRST provides guarantees about **testing behavior**, not about application correctness by itself.

Specifically, FIRST guarantees that:

- **Crashes are deterministic and replayable**  
  Given the same test, crash schedule, and seed, failures can be reproduced exactly.

- **Crash points are explicit and observable**  
  Tests run with well-defined crash boundaries rather than implicit assumptions about execution.

- **Recovery logic is always exercised**  
  Every injected crash is followed by a restart and execution of user-provided recovery code.

- **Persistent state after crashes is inspectable**  
  Filesystem state can be observed, compared, and validated after each crash.

- **Invariant violations are surfaced early**  
  Violations appear as minimal, reproducible test failures rather than intermittent production bugs.

FIRST does **not** guarantee that a system is correct.

Instead, it guarantees that:
- Crash behavior is systematically explored
- Recovery paths are continuously tested
- Violations are reproducible and debuggable

Correctness remains the responsibility of the storage system and the invariants it defines.

## Non-Goals

FIRST is intentionally focused and does not attempt to solve every testing problem.

Specifically, FIRST is **not**:

- A fuzzer or property-based testing framework  
  FIRST prioritizes determinism and systematic exploration over randomness.

- A performance or benchmarking tool  
  FIRST focuses exclusively on correctness under crashes, not throughput or latency.

- A database or storage engine  
  FIRST provides testing infrastructure, not storage primitives or recovery logic.

- A replacement for unit or integration tests  
  FIRST complements existing testing by targeting crash and recovery paths.

- A distributed systems testing framework  
  FIRST targets local persistence and crash recovery, not network partitions or consensus failures.

These non-goals are deliberate.

By remaining narrowly scoped, FIRST aims to be reliable, predictable, and easy to integrate into existing storage systems.

## Minimal Example

The following example illustrates how FIRST is intended to be used.

A storage system provides normal write and recovery logic. FIRST controls crash injection, restart, and invariant validation around it.

```rust
use first::test;

test(|env| {
    let db = MyDb::open(env.path());

    db.put("a", "1");
    db.put("b", "2");

    // FIRST may crash the process at any point above.
    // After restart, recovery logic is executed automatically.

    db.assert_invariants();
});

## Project Status

FIRST is in an early stage of development.

The current focus is on:
- Defining a clear and minimal core model
- Establishing deterministic crash and recovery semantics
- Validating the design through small, targeted examples

The API is intentionally unstable and subject to change.

This early phase prioritizes:
- Correctness over completeness
- Clarity over feature count
- Design rigor over rapid expansion

Feedback from storage and database engineers is welcome, particularly around:
- Invariant modeling
- Crash boundaries
- Recovery semantics

## Roadmap

FIRST is being developed in deliberate phases.

### Phase 1 — Core crash testing (current focus)

- Deterministic crash scheduler
- Explicit crash points
- Process termination and restart
- Persistent state inspection
- Minimal example storage systems

### Phase 2 — Reusable test harness

- Ergonomic test API
- Clear separation between test logic and storage logic
- Invariant definition helpers
- Improved diagnostics for failure reproduction

### Phase 3 — Language bindings

- Rust (native)
- C / C++
- Go
- Java

The roadmap is intentionally conservative.

Features are added only when they preserve determinism, reproducibility, and clarity.

## Contributing

FIRST is an early-stage project focused on correctness, determinism, and clear design.

Contributions are welcome, especially in the form of:
- Design feedback
- Invariant modeling ideas
- Small, focused examples
- Documentation improvements

Before contributing code, please open an issue to discuss the approach.

This helps keep the core design coherent and avoids premature complexity.

See `CONTRIBUTING.md` for details.

