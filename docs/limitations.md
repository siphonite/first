# FIRST Limitations

This document describes known limitations of the FIRST framework.

---

## Crash Model Limitation (v0.1)

FIRST v0.1 models process crashes using `SIGKILL`.

### What v0.1 Accurately Simulates

- Sudden process termination
- Loss of in-memory state
- Incomplete syscalls (operations interrupted mid-execution)
- Recovery from filesystemstate left by prior operations

### What v0.1 Does NOT Simulate

- **Power loss** — Actual power failure affects the block layer and hardware
- **Block-layer rollback** — Storage devices may lose data in their write caches
- **Loss of kernel metadata already applied** — e.g., directory entries that are in the page cache but not yet journaled

### Implications

As a result, certain real-world crash-consistency bugs cannot be exposed in v0.1:

| Bug Type | Detectable in v0.1? | Notes |
|----------|---------------------|-------|
| Missing `fsync()` after data write | ⚠️ Partial | Only if kernel hasn't flushed page cache |
| Missing parent-directory `fsync()` after `rename()` | ❌ No | `rename()` is applied at kernel level before crash point |
| Partial sector writes | ❌ No | Requires block-layer simulation |
| Torn writes | ❌ No | Requires hardware-level simulation |
| Recovery logic bugs | ✅ Yes | SIGKILL accurately tests recovery paths |
| Transaction atomicity bugs | ✅ Yes | Visible through invariant checking |

### Example: Why Parent Directory Fsync Bugs Are Not Detectable

Consider this code:

```rust
fs::rename(&tmp_path, &wal_path)?;  // Kernel applies rename immediately
crash_point("after_rename");        // SIGKILL here
// Missing: fsync parent directory
```

When `crash_point` triggers `SIGKILL`:
1. The `rename()` syscall has **already completed** at the kernel level
2. The directory entry exists in the kernel's VFS cache
3. `SIGKILL` terminates the process but does **not** undo kernel operations
4. Recovery sees the renamed file as expected

The bug would only manifest if:
- Power was lost before the directory entry was journaled
- The filesystem replayed its journal without the rename

### Scope for Milestone 3

The "missing parent-directory fsync" bug is **explicitly out of scope** for the Milestone 3 proof.

Milestone 3 demonstrates:
- FIRST's orchestration model works correctly
- Crash points are injected and triggered deterministically
- Recovery and invariant checking function as designed
- The framework is ready for enhanced crash simulation

### Future Versions

Future versions of FIRST may add:

1. **Filesystem interposition (v0.2)**
   - `LD_PRELOAD` to intercept syscalls
   - Simulate incomplete `rename()` or `write()` operations

2. **FUSE-based simulation**
   - Virtual filesystem that can selectively lose data
   - Full control over what survives a crash

3. **Block-layer simulation**
   - Intercept at the device level
   - Simulate write cache behavior and torn writes

---

## Other Known Limitations

### Single-Process Only (v0.1)

FIRST v0.1 runs tests in a single-process model. Multi-process or distributed scenarios are not supported.

### No Concurrency Testing (v0.1)

Crash points are serialized. Concurrent operations are not explored for race conditions related to persistence.

### Linux Only (v0.1)

The current implementation uses Linux-specific APIs (`SIGKILL`, `/proc`). macOS and Windows support is not yet available.
