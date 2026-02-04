# Architecture Decision: `crash_point()` Semantics

## 1. Overview

`first::crash_point()` is the core primitive for explicit crash injection. This document defines its exact behavior for v0.1.

---

## 2. Function Signature

```rust
/// A potential crash location in the execution.
/// 
/// When FIRST is active during the "Execution" phase, this function:
/// 1. Increments the global crash counter.
/// 2. If `counter == target`, terminates the process immediately.
/// 3. Otherwise, returns normally.
/// 
/// When FIRST is inactive (Orchestrator or Verify phase), this is a no-op.
pub fn crash_point(label: &str);
```

**Labels are required.** This ensures:
- Every crash point is identifiable in logs/reports.
- Developers think intentionally about where crashes matter.
- Future features (e.g., filtering by label) are possible.

---

## 3. Counter Increment Timing

The counter increments **at the start of `crash_point()`**, before any check.

```
crash_point("A")   → Counter: 0 → 1. Check: 1 == target?
crash_point("B")   → Counter: 1 → 2. Check: 2 == target?
crash_point("C")   → Counter: 2 → 3. Check: 3 == target?
```

**Rationale:** Incrementing first means "crash point N" refers to "crash *before* the Nth point completes." This models "power loss during operation N."

---

## 4. Crash Metadata

When a crash is triggered, the following is recorded (written to a file or stderr before `SIGKILL`):

| Field | Description | Example |
|-------|-------------|---------|
| `point_id` | The numeric crash counter value | `42` |
| `label` | The string passed to `crash_point()` | `"after_wal_write"` |
| `seed` | The test's random seed (if any) | `12345` |
| `work_dir` | Path to the test's isolated directory | `/tmp/first/run_42` |

**Format (JSON to stderr before kill):**
```json
{"event":"crash","point_id":42,"label":"after_wal_write","seed":12345,"work_dir":"/tmp/first/run_42"}
```

The Orchestrator parses this from the child's stderr after `SIGKILL`.

---

## 5. Target Not Reached

If the "Execution" phase completes *without* the target crash point being reached:

1. The child process exits normally (exit code 0).
2. The Orchestrator detects this.
3. **Interpretation:** The schedule is exhausted. All crash points have been explored.
4. The test passes (assuming all previous iterations passed).

**Edge Case:** If `target == 0`, we crash immediately at the first `crash_point()` call.

---

## 6. Determinism Guarantees (v0.1)

For v0.1, we guarantee:

| Property | Guarantee |
|----------|-----------|
| **Same crash point** | Given the same `target`, the crash occurs at the same logical location. |
| **Same label** | The label is always the one passed to the matching `crash_point()` call. |
| **Same filesystem state** | If the workload is deterministic, the FS state at crash is identical across runs. |

**NOT Guaranteed (v0.1):**
- Thread interleaving (single-threaded model assumed).
- Timing of background OS operations (page cache flush).

---

## 7. No-Op Behavior

When `FIRST_PHASE` is not `EXECUTION`, `crash_point()` does nothing:

```rust
pub fn crash_point(label: &str) {
    if !is_execution_phase() {
        return; // No-op: Orchestrator or Verify phase
    }
    // ... increment and check ...
}
```

This allows the same test code to run in all phases without conditional compilation.

---

## 8. Summary

| Question | Answer |
|----------|--------|
| When does counter increment? | At the *start* of `crash_point()`, before the check. |
| Are labels required? | **Yes**, always required. |
| What metadata is recorded? | `point_id`, `label`, `seed`, `work_dir` (JSON to stderr). |
| What if target not reached? | Child exits 0; Orchestrator treats it as "schedule exhausted." |
| Determinism for v0.1? | Same target → same crash location & FS state (single-threaded). |

---

## 9. Open Questions (for later)

- Should we support `crash_point_if(condition, label)` for conditional crashes?
- Should labels be globally unique, or can they repeat (e.g., in a loop)?
- Should we log *all* crash points encountered, not just the triggered one?

These can be deferred to v0.2+.
