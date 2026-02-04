# FIRST Limitations

This document describes known limitations of the FIRST framework.

---

## Crash Model (v0.1)

FIRST v0.1 models process crashes using `SIGKILL`.

### What v0.1 Accurately Simulates

- Sudden process termination
- Loss of in-memory state
- Incomplete syscalls (operations interrupted mid-execution)
- Recovery from filesystem state left by prior operations
- Page cache flush ordering non-determinism

### What v0.1 Does NOT Simulate

- **Power loss** — Actual power failure affects the block layer and hardware
- **Block-layer rollback** — Storage devices may lose data in their write caches
- **Torn writes** — Partial sector writes that can occur during power loss
- **Filesystem metadata rollback** — Directory entries applied but not journaled

### Detectable Bug Classes

| Bug Type | Detectable? | Notes |
|----------|-------------|-------|
| Recovery logic errors | ✅ Yes | SIGKILL accurately tests recovery paths |
| Transaction atomicity bugs | ✅ Yes | Commit-before-durable patterns exposed |
| Missing fsync before commit | ✅ Yes | Page cache flush ordering is undefined |
| Missing parent-directory fsync | ❌ No | Requires filesystem interposition |
| Partial sector writes | ❌ No | Requires block-layer simulation |
| Torn writes | ❌ No | Requires hardware-level simulation |

---

## Reference WAL Bug (Intentional)

The reference WAL contains one intentional recovery-semantic bug to serve as proof that FIRST can expose real crash-consistency violations.

### Bug Description

The WAL writes the transaction `COMMIT` marker before ensuring all transaction records are durable:

```
write(record_1)
write(record_2)
write(record_3)
write(COMMIT)
fsync()         ← records and commit flushed together, order undefined
```

The correct implementation would be:

```
write(record_1)
write(record_2)
write(record_3)
fsync()         ← ensure records are durable
write(COMMIT)
fsync()         ← then write commit
```

### Failure Mode

When a crash occurs at `after_commit_write` (after COMMIT is written but before fsync):

1. Both records and COMMIT are in the kernel page cache
2. The process is killed with SIGKILL
3. The kernel flushes the page cache in undefined order
4. COMMIT may reach disk before all records
5. Recovery sees a committed transaction with missing records
6. **Atomicity invariant violated**

### Why This Bug Is Valid Under SIGKILL

This bug does not require power loss or block-layer simulation. It relies only on:

- **Page cache behavior**: Writes go to page cache, not directly to disk
- **Flush ordering**: Kernel may flush pages in any order
- **Missing durability barrier**: No fsync between records and COMMIT

When SIGKILL terminates the process, the kernel continues to manage the page cache. The order in which dirty pages are written to disk is not guaranteed to match the order they were written by the application.

---

## Proof of FIRST Effectiveness (v0.1)

The following proof chain demonstrates that FIRST v0.1 can expose real crash-consistency bugs:

1. **FIRST injects a crash** at the `after_commit_write` crash point
2. **The process is killed** with SIGKILL
3. **Filesystem state is preserved** exactly as it existed at crash time
4. **Recovery runs** and opens the WAL
5. **Recovery observes** a `COMMIT` marker for transaction 1
6. **Recovery attempts** to replay the transaction
7. **Some records are missing** (not yet flushed to disk)
8. **Atomicity invariant fails**: committed transaction has partial visibility
9. **Failure is deterministic** and reproducible with the same crash point

This proof validates:

- Crash point injection works correctly
- SIGKILL-based process termination preserves filesystem state
- Recovery logic is exercised after each crash
- Invariant checking detects atomicity violations
- Failures are reproducible for debugging

---

## Future Versions

Future versions of FIRST may add enhanced crash simulation:

### v0.2 — Filesystem Interposition

- `LD_PRELOAD` to intercept syscalls
- Simulate incomplete `rename()` or `write()` operations
- Expose directory-fsync bugs

### v0.3 — FUSE-based Simulation

- Virtual filesystem with selective data loss
- Full control over what survives a crash

### v0.4 — Block-layer Simulation

- Device-level interception
- Simulate write cache behavior and torn writes

---

## Other Known Limitations

### Single-Process Only (v0.1)

FIRST v0.1 runs tests in a single-process model. Multi-process or distributed scenarios are not supported.

### No Concurrency Testing (v0.1)

Crash points are serialized. Concurrent operations are not explored for race conditions related to persistence.

### Linux Only (v0.1)

The current implementation uses Linux-specific APIs (`SIGKILL`, `/proc`). macOS and Windows support is not yet available.
