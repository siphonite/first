# ADR-002: `crash_point()` Semantics

**Status:** Accepted

## Function Signature

```rust
pub fn crash_point(label: &str);
```

**Labels are required** — ensures every crash point is identifiable in logs and failure reports.

## Behavior

| Phase | Behavior |
|-------|----------|
| Execution | Increments counter, may terminate process |
| Orchestrator | No-op |
| Verify | No-op |

## Counter Semantics

Counter increments **before** the target check (1-indexed):

```
crash_point("A")  → Counter: 0 → 1. Check: 1 == target?
crash_point("B")  → Counter: 1 → 2. Check: 2 == target?
```

## Crash Metadata

When triggered, emits JSON to stderr before `SIGKILL`:

```json
{"event":"crash","point_id":5,"label":"after_commit","seed":null,"work_dir":"/tmp/first/run_5"}
```

## Exit Behavior

| Condition | Exit Code | Orchestrator Interpretation |
|-----------|-----------|----------------------------|
| Target reached | 137 (SIGKILL) | Expected crash → run VERIFY |
| Target not reached | 0 | Schedule exhausted → success |

## Determinism Guarantees

| Guaranteed | Not Guaranteed |
|------------|----------------|
| Same target → same crash location | Thread interleaving |
| Same label | OS page cache timing |
| Same FS state (if workload is deterministic) | |
