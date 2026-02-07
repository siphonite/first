# ADR-004: Orchestrator

**Status:** Accepted

## Core Loop

```
target = 1
loop {
    1. Create /tmp/first/run_{target}
    2. Spawn EXECUTION child
    3. Wait for exit:
       - 137 (SIGKILL) → run VERIFY
       - 0 → done (schedule exhausted)
       - other → failure
    4. Spawn VERIFY child
    5. If VERIFY fails → stop
    6. target += 1
}
```

## Decisions

| Aspect | Choice |
|--------|--------|
| Discovery | Iterative (no pre-counting) |
| Filesystem | Fresh directory per target |
| Self-spawning | `std::env::current_exe()` |
| Crash detection | Exit code 137 |
| Cleanup | Delete on success (keep on failure) |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `FIRST_PHASE` | `EXECUTION` / `VERIFY` |
| `FIRST_CRASH_TARGET` | Target crash point (1-indexed) |
| `FIRST_WORK_DIR` | Isolated directory |
| `FIRST_SEED` | Random seed |
| `FIRST_KEEP_ARTIFACTS` | Set to `1` to preserve dirs |

## Exit Codes

| Code | Meaning | Action |
|------|---------|--------|
| 137 | SIGKILL | Run VERIFY |
| 0 | Normal | Schedule exhausted |
| 101 | Panic | Test failure |

## API

```rust
first::test()
    .run(|env| { /* workload */ })
    .verify(|env, crash| { /* invariants */ })
    .execute();
```

## Deferred (v0.2+)

- CoW snapshots
- Parallel execution  
- Crash pruning
- Windows/macOS
