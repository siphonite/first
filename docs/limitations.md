# FIRST Limitations

## Crash Model (v0.1)

FIRST uses `SIGKILL` to simulate crashes.

### What It Simulates

✅ Sudden process termination  
✅ Loss of in-memory state  
✅ Recovery from filesystem state  
✅ Page cache flush ordering non-determinism  

### What It Does NOT Simulate

❌ Power loss (block-layer effects)  
❌ Torn writes (partial sector writes)  
❌ Filesystem metadata rollback  

### Detectable Bugs

| Bug Type | Detectable |
|----------|------------|
| Recovery logic errors | ✅ |
| Transaction atomicity bugs | ✅ |
| Missing fsync before commit | ✅ |
| Missing parent-directory fsync | ❌ |
| Torn writes | ❌ |

---

## Integration Constraints

| Constraint | Status |
|------------|--------|
| One `first::test()` per `#[test]` | Required |
| Single-threaded `.run()` closure | Required |
| `#[tokio::test]` / async | ❌ Not supported |
| `crash_point()` from spawned threads | ❌ Undefined |
| Nested workspaces | ❌ Not supported |

---

## Platform Support

| Platform | Status |
|----------|--------|
| Linux | ✅ Supported |
| macOS | ❌ Planned |
| Windows | ❌ Planned |

---

## Roadmap

| Version | Feature |
|---------|---------|
| v0.2 | `LD_PRELOAD` syscall interception |
| v0.3 | FUSE-based simulation |
| v0.4 | Block-layer simulation |
