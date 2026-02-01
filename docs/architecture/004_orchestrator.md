# Architecture Decision: Test Runner and Orchestration Loop

## 1. Overview

This document defines the orchestration engine for FIRST v0.1 — the supervisor that turns one test function into N crash-verify cycles.

---

## 2. Core Loop

```
target = 1
loop {
    1. Create work_dir: /tmp/first/run_{target}
    2. Spawn EXECUTION child with target
    3. Wait for exit:
       - SIGKILL (137) → crash occurred, run VERIFY
       - Exit 0 → schedule exhausted, DONE
       - Other → test failure, STOP
    4. Spawn VERIFY child against same work_dir
    5. If VERIFY fails → STOP with failure
    6. target += 1
}
```

---

## 3. Locked Decisions (v0.1)

| Decision | Choice |
|----------|--------|
| Discovery | Iterative (no pre-counting) |
| Filesystem | Fresh directory per target |
| Phase selection | Environment variables |
| Self-spawning | `std::env::current_exe()` |
| Crash detection | Exit code 137 (SIGKILL) |
| Cleanup on success | Delete (unless `FIRST_KEEP_ARTIFACTS=1`) |
| Cleanup on failure | **Always keep** |

---

## 4. Test Name Filtering

**Constraint:** FIRST v0.1 assumes one FIRST test per Rust test function.

**Mechanism:**
1. Orchestrator extracts test name from `std::env::args()` or env var
2. Children are spawned with: `cargo test <test_name> -- --exact`

**Environment variable:**
```
FIRST_TEST_NAME=<exact test name>
```

---

## 5. Environment Variables

| Variable | Phase | Description |
|----------|-------|-------------|
| `FIRST_PHASE` | All | `EXECUTION` / `VERIFY` (default: Orchestrator) |
| `FIRST_CRASH_TARGET` | Execution | Target crash point ID (1-indexed) |
| `FIRST_WORK_DIR` | All | Isolated directory for this run |
| `FIRST_SEED` | All | Random seed for reproducibility |
| `FIRST_TEST_NAME` | Children | Exact test name to run |
| `FIRST_KEEP_ARTIFACTS` | Orchestrator | Set to `1` to keep dirs on success |

---

## 6. Exit Code Interpretation

| Exit Code | Meaning | Orchestrator Action |
|-----------|---------|---------------------|
| 137 | SIGKILL (crash occurred) | Run VERIFY phase |
| 0 | Normal completion | Schedule exhausted (success) |
| 101 | Rust panic | Test failure |
| Other | Unexpected | Test failure |

---

## 7. Progress Output

Minimal, one line per crash point:

```
[first] crash point 1: OK
[first] crash point 2: OK
[first] crash point 3: FAILED (see /tmp/first/run_3)
```

No spinners, percentages, or verbosity flags.

---

## 8. API Shape

```rust
first::test()
    .run(|env| {
        // Workload: executed in EXECUTION phase only
    })
    .verify(|env, crash_info| {
        // Recovery + invariants: executed in VERIFY phase only
    });
```

**Behavior by phase:**

| Phase | `.run()` | `.verify()` |
|-------|----------|-------------|
| Orchestrator | Stored, not called | Stored, not called |
| Execution | **Called** | Ignored |
| Verify | Ignored | **Called** |

---

## 9. Components to Implement

| Component | Purpose | File |
|-----------|---------|------|
| `TestBuilder` | Stores closures, detects mode | `src/test.rs` |
| `Env` | Provides `path()` to closures | `src/env.rs` |
| `CrashInfo` | Parsed crash metadata | `src/env.rs` |
| `Orchestrator` | The supervisor loop | `src/orchestrator.rs` |
| `spawn_child()` | Re-invokes with env vars | `src/orchestrator.rs` |

---

## 10. Explicitly Deferred (NOT v0.1)

- Snapshots / CoW filesystem
- Parallel crash runs
- Crash pruning
- Multiple FIRST tests per function
- Structured output formats (JSON)
- Windows/macOS support
