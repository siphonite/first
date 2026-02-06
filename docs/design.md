# FIRST Design Document

**FIRST** (Filesystem Injection and Reliability Stress Test) is a deterministic crash testing framework for storage engines and WAL-based systems.

---

## Table of Contents

1. [Problem Statement](#1-problem-statement)
2. [Goals and Non-Goals](#2-goals-and-non-goals)
3. [Core Concepts](#3-core-concepts)
4. [Execution Model](#4-execution-model)
5. [API Design](#5-api-design)
6. [Implementation Approach](#6-implementation-approach)
7. [State Space and Performance](#7-state-space-and-performance)
8. [Invariants and Validation](#8-invariants-and-validation)
9. [Concurrency Model](#9-concurrency-model)
10. [Comparison to Existing Work](#10-comparison-to-existing-work)
11. [Limitations and Future Work](#11-limitations-and-future-work)

---

## 1. Problem Statement

Modern databases and storage engines rely on strong crash-consistency guarantees. They write data using WALs, manifests, page files, and metadata updates, then recover state after crashes by replaying logs or rebuilding in-memory structures.

**The challenge:** Recovery paths are some of the most critical—and most fragile—code in the system.

### Where Traditional Testing Fails

Across the ecosystem today:

- **Unit tests** validate logic assuming uninterrupted execution
- **Integration tests** exercise happy paths
- **Benchmarks** measure performance, not correctness
- **Fault injection** exists, but is often coarse or non-deterministic

**Real failures do not happen at function boundaries.**

They happen:
- Between writes
- During system calls
- After data reaches the page cache but before persistence
- After a WAL append but before a commit marker
- After a rename but before directory fsync

These failure modes are difficult to reason about and easy to miss. As a result, many storage systems ship with latent crash-consistency bugs that pass all tests and surface only under real crashes, often years later.

### The Industry Pattern

Every major database eventually builds internal crash-testing infrastructure. But these solutions are:

- Reinvented independently
- Tightly coupled to internal codebases
- Hard to maintain
- Rarely exhaustive
- Not reusable across projects

**There is no standard framework, shared language, or common methodology for testing crash correctness in storage systems.**

FIRST exists to fill this gap.

---

## 2. Goals and Non-Goals

### Goals

FIRST is designed to make crash consistency a **testable and repeatable property** of storage systems.

**Primary goals:**

1. **Deterministic crash testing**  
   Crashes must be injected at precise, replayable points so that failures can be reproduced exactly.

2. **Micro-scale precision**  
   Crash points are injected at precise logical steps (e.g., "after write, before fsync"), not random time intervals.

3. **Systematic exploration of crash boundaries**  
   Focus on explicit, high-value crash boundaries defined by persistence and recovery semantics.

4. **Recovery-centric validation**  
   Every crash is followed by a restart and execution of recovery logic, ensuring recovery paths are continuously exercised.

5. **Invariant-driven correctness**  
   FIRST validates storage systems by asserting invariants that must hold after crashes, not by comparing expected outputs.

6. **Language agnostic architecture**  
   While the initial implementation is Rust, the design generalizes to C++/Go storage engines.

### Non-Goals

FIRST intentionally avoids several classes of problems to remain focused and predictable.

**Explicitly NOT goals:**

1. **Distributed systems testing**  
   FIRST targets single-node persistence consistency. It is not Jepsen.

2. **Hardware fault simulation**  
   We do not simulate bit flips, disk latency, or partial sector writes. We simulate power loss (clean stops).

3. **Concurrency fuzzing**  
   FIRST initially serializes execution. It is not a thread sanitizer.

4. **Performance or benchmarking**  
   FIRST focuses exclusively on correctness under crashes, not throughput or latency.

5. **Exhaustive failure coverage**  
   FIRST does not attempt to explore all possible interleavings; it prioritizes meaningful and reproducible crash scenarios.

These constraints are deliberate and allow FIRST to remain deterministic, debuggable, and usable in practice.

---

## 3. Core Concepts

FIRST models a storage system as a **state machine** transitioning between persistent states via atomic operations.

### 3.1 The Persistence Boundary

The **Persistence Boundary** is the interface between the application's volatile memory and persistent storage media. FIRST intercepts operations at this boundary to control the "moment of failure."

```
┌─────────────────────────────────────────────────┐
│ Application Logic                               │
│ • In-memory state                               │
│ • Control flow                                  │
│ • Data structures                               │
│                                                 │
│   ┌─────────────────────────────────────────┐   │
│   │ Persistence Boundary  ← FIRST operates  │   │
│   │ • write(), pwrite()                     │   │
│   │ • fsync(), fdatasync()                  │   │
│   │ • rename(), ftruncate()                 │   │
│   │ • unlink(), msync()                     │   │
│   │                                         │   │
│   │   ┌─────────────────────────────────┐   │   │
│   │   │ Durable State                   │   │   │
│   │   │ • Files and directories         │   │   │
│   │   │ • Persists across crashes       │   │   │
│   │   └─────────────────────────────────┘   │   │
│   └─────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

**Included operations:**
- `write()`, `pwrite()` - Data writes
- `fsync()`, `fdatasync()` - Durability barriers
- `rename()`, `ftruncate()` - Metadata operations
- `unlink()` - Deletion
- `msync()` - Memory-mapped file sync

**Excluded (treated as volatile):**
- `mmap()` mutations without explicit `msync()`
- In-memory buffering
- OS page cache (until fsync)

This boundary model allows FIRST to focus on **logical persistence semantics** rather than low-level hardware behavior.

### 3.2 Crash Points

A **Crash Point** is a specific logical location in the code where the system might lose power.

FIRST treats crashes as **first-class citizens** in execution, not rare external events.

**Two types of crash points:**

1. **Implicit** - Before/after every syscall in the Persistence Boundary
2. **Explicit** - Manually annotated points where developers want to test atomicity

```
Timeline of Execution with Crash Points:

Normal Code → write("a") → [CP1] → fsync() → [CP2] → write("b") → [CP3] → fsync() → [CP4]
              ↑            ↑        ↑         ↑        ↑            ↑        ↑         ↑
              operation    crash    operation crash   operation    crash    operation crash
                          point              point                 point              point
```

**Example with explicit annotation:**

```rust
db.put("key1", "value1");           // Implicit CP after write()
first::crash_point("after_key1");   // Explicit CP (named)
db.put("key2", "value2");           // Implicit CP after write()
db.sync();                          // Implicit CP after fsync()
first::crash_point("committed");    // Explicit CP (named)
```

### 3.3 Crash Schedules

A **Crash Schedule** is a deterministic plan that dictates:

- Which operations proceed normally
- At precisely which crash point the system halts
- (Future) Which subset of unsynced data is lost vs. persisted

**Schedule representation:**

```
Schedule Format: [crash_point_id]

Example: [42]
→ Execute normally until crash point #42
→ Crash immediately at that point
→ Restart and validate recovery
```

**Deterministic generation from seed:**

```
Seed: 12345
↓
Generated schedule: [0, 5, 12, 18, 23, ...]
↓
Test runs in sequence, one crash point per execution
```

This approach ensures:
- **Reproducibility** - Same seed = same crash sequence
- **Debuggability** - Failed tests include the exact crash point
- **No flakiness** - Deterministic, not probabilistic

### 3.4 Invariants Over Outputs

FIRST validates correctness through **invariants** - properties that must hold after recovery, regardless of when a crash occurs.

**Invariants describe truths, not expected outputs.**

Traditional testing:
```rust
// Traditional approach - compare outputs
assert_eq!(db.get("key"), Some("value"));  // Fragile!
```

FIRST approach:
```rust
// Invariant-based - assert properties
assert!(db.is_internally_consistent());     // Must always hold
assert!(committed_data_exists || db.is_empty());  // Logical truth
```

**Common invariants for storage systems:**

1. **Atomicity** - Transactions are fully committed or fully rolled back
2. **Durability** - Data acknowledged as synced must exist after recovery
3. **Integrity** - Checksums match, pointers are valid
4. **Monotonicity** - Sequence numbers never decrease
5. **Consistency** - Internal structures (B-trees, manifests) are valid

---

## 4. Execution Model

FIRST executes storage tests as a **sequence of controlled executions** separated by crashes and restarts.

### 4.1 Test Lifecycle

Each test proceeds through the following phases:

```
┌──────────────────┐
│   Test Start     │
│ (clean state)    │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Normal Execution │ ← Workload runs
│ (run to crash    │   FIRST monitors persistence ops
│  point N)        │   Increments crash point counter
└────────┬─────────┘
         │
         │ Crash point N reached
         │ according to schedule
         ▼
┌──────────────────┐
│  Process Crash   │ ← Immediate termination
│  (SIGKILL)       │   No cleanup, no destructors
│                  │   In-memory state lost
└────────┬─────────┘
         │
         │ Filesystem state
         │ preserved exactly
         ▼
┌──────────────────┐
│ Restart & Open   │ ← New process starts
│ Persistent State │   Opens preserved FS state
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Recovery Logic   │ ← System-specific recovery
│ (WAL replay,     │   FIRST does NOT interpret
│  index rebuild)  │   System owns this logic
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Invariant Checks │ ← Validate recovered state
│ (user-defined)   │   Assert properties hold
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Next Crash Point │ ← Repeat with next schedule entry
│ (if any remain)  │
└──────────────────┘
```

### 4.2 Responsibility Boundaries

FIRST owns the **execution lifecycle**, crash timing, and restart orchestration.

The **system under test** owns:

- Normal operation logic
- Persistent data layout
- Recovery procedures
- Invariant definitions

This separation allows storage systems to be tested without modifying their core logic to accommodate crash testing.

```
┌───────────────────────────────────────┐
│            FIRST Framework            │
│                                       │
│  ┌─────────────────────────────────┐  │
│  │ • Crash injection               │  │
│  │ • Schedule management           │  │
│  │ • Process restart               │  │
│  │ • FS state preservation         │  │
│  │ • Invariant runner              │  │
│  └─────────────────────────────────┘  │
└───────────────────────────────────────┘
           ↕ (well-defined API)
┌───────────────────────────────────────┐
│       System Under Test (e.g., DB)    │
│                                       │
│  ┌─────────────────────────────────┐  │
│  │ • Write operations              │  │
│  │ • WAL/manifest logic            │  │
│  │ • Recovery implementation       │  │
│  │ • Invariant definitions         │  │
│  └─────────────────────────────────┘  │
└───────────────────────────────────────┘
```

### 4.3 Determinism Guarantees

Given the same:
- Test code
- Initial persistent state
- Crash schedule (seed)
- Execution environment

FIRST guarantees:
- Crashes occur at the same logical points
- Persistent state after each crash is identical
- Recovery behavior is repeatable

**What FIRST controls:**
- Crash point selection and timing
- Filesystem state snapshots
- Recovery invocation order

**What FIRST does NOT control:**
- CPU scheduling (not needed for persistence correctness)
- OS-level timing
- External system interactions

This balance preserves realism while keeping failures debuggable.

---

## 5. API Design

FIRST provides a clean, ergonomic API that separates test workload from recovery validation.

### 5.1 Complete Example

```rust
use first::test;

/// Test an append-only log for crash consistency
fn test_append_log() {
    first::test()
        // Define the workload to execute
        .run(|env| {
            let log = AppendLog::create(env.path("mylog"));
            
            // Crash Point 0: After file creation (implicit)
            
            log.append("entry1").expect("append failed");
            // Crash Point 1: After write() for entry1 (implicit)
            
            first::crash_point("after_entry1");  
            // Crash Point 2: Explicit named point
            
            log.sync().expect("sync failed");
            // Crash Point 3: After fsync() (implicit)
            
            log.append("entry2").expect("append failed");
            // Crash Point 4: After write() for entry2 (implicit)
            
            log.sync().expect("sync failed");
            // Crash Point 5: After final fsync() (implicit)
        })
        
        // Define recovery and validation
        .verify(|env, crash_info| {
            println!("Recovering from crash at: {:?}", crash_info);
            
            // System restarts against the FS state from crash moment
            let log = AppendLog::open(env.path("mylog"))
                .expect("log should always be openable");
            
            // Define invariants that MUST hold
            let entries = log.read_all();
            
            match entries.len() {
                // Crashed before first sync - no data persisted
                0 => {
                    assert!(entries.is_empty());
                },
                
                // Crashed after first sync, before second
                1 => {
                    assert_eq!(entries, vec!["entry1"]);
                },
                
                // Crashed after both syncs
                2 => {
                    assert_eq!(entries, vec!["entry1", "entry2"]);
                },
                
                // Invalid state - atomicity violated!
                _ => {
                    panic!("Invalid state: {:?}", entries);
                }
            }
            
            // Additional invariants
            assert!(log.verify_checksums(), "Checksums must be valid");
            assert!(log.is_well_formed(), "Structure must be consistent");
        });
}
```

### 5.2 API Components

**Test setup:**
```rust
first::test()
    .seed(12345)              // Optional: deterministic seed
    .max_crashes(100)         // Optional: limit crash points to explore
    .run(|env| { ... })       // Workload execution
    .verify(|env, info| { ... }) // Recovery and validation
```

**Environment API:**
```rust
env.path("dir")               // Path inside workspace for persistent state
env.crash_point_count()       // Number of crash points discovered
```

**Crash point annotation:**
```rust
first::crash_point("label")   // Explicit crash point with name
```

**Crash info in verification:**
```rust
crash_info.point_id           // Numeric crash point ID
crash_info.label              // Optional label (if explicit)
crash_info.operation          // Which syscall was interrupted (if any)
```

### 5.3 Multi-Test Example

```rust
/// Test WAL atomicity
fn test_wal_atomicity() {
    first::test()
        .run(|env| {
            let wal = WAL::create(env.path("wal"));
            
            // Begin transaction
            let tx = wal.begin_tx();
            tx.write("key1", "value1");
            tx.write("key2", "value2");
            
            first::crash_point("before_commit");
            
            // Commit writes to WAL
            tx.commit();
            
            first::crash_point("after_commit");
        })
        .verify(|env, _| {
            let wal = WAL::open(env.path("wal"));
            let entries = wal.replay();
            
            // Invariant: Transaction is atomic
            // Either both writes are present, or neither
            assert!(
                entries.is_empty() || 
                entries == vec![("key1", "value1"), ("key2", "value2")]
            );
        });
}

/// Test manifest consistency
fn test_manifest_update() {
    first::test()
        .run(|env| {
            let db = Database::create(env.path("db"));
            
            // Generate SSTable file
            let sst = db.flush_memtable();
            
            first::crash_point("sst_written");
            
            // Update manifest to reference new SSTable
            db.manifest_add(sst);
            
            first::crash_point("manifest_updated");
        })
        .verify(|env, _| {
            let db = Database::open(env.path("db"));
            
            // Invariant: No orphaned SSTable files
            let referenced = db.manifest_list_sstables();
            let on_disk = db.scan_sstable_files();
            
            for file in &on_disk {
                assert!(
                    referenced.contains(file),
                    "SSTable {:?} not in manifest - orphaned file!",
                    file
                );
            }
            
            // Invariant: No dangling references
            for file in &referenced {
                assert!(
                    on_disk.contains(file),
                    "Manifest references {:?} but file missing!",
                    file
                );
            }
        });
}
```

---

## 6. Implementation Approach

This section details how FIRST actually works under the hood.

### 6.1 Crash Injection Mechanism

FIRST supports two modes of crash injection, with different tradeoffs.

#### Option A: Explicit Annotation (Primary/v1)

The application calls `first::crash_point(name)` directly.

**Mechanism:**
```rust
// Global state in FIRST runtime
static CRASH_COUNTER: AtomicUsize = AtomicUsize::new(0);
static TARGET_CRASH_POINT: AtomicUsize = AtomicUsize::new(usize::MAX);

pub fn crash_point(label: &str) {
    let current = CRASH_COUNTER.fetch_add(1, Ordering::SeqCst);
    
    if current == TARGET_CRASH_POINT.load(Ordering::SeqCst) {
        // Log the crash point for reproducibility
        eprintln!("FIRST: Crashing at point {} ({})", current, label);
        
        // Immediate termination (see §6.3)
        unsafe { libc::kill(std::process::id() as i32, libc::SIGKILL) };
    }
}
```

**Advantages:**
- Simple and robust
- No syscall interception complexity
- Works across all platforms
- Developer has full control over crash granularity

**Disadvantages:**
- Requires source modification
- May miss implicit crash points if not annotated thoroughly

#### Option B: Syscall Interception (Planned/v2)

FIRST intercepts persistence syscalls transparently using `LD_PRELOAD` (Linux) or similar.

**Mechanism:**
```c
// Wrapper in libfirst_intercept.so
ssize_t write(int fd, const void *buf, size_t count) {
    first_crash_point_check("write");  // May crash here
    
    // Call real write
    ssize_t result = real_write(fd, buf, count);
    
    first_crash_point_check("write_after");  // May crash after
    
    return result;
}

int fsync(int fd) {
    first_crash_point_check("fsync");
    int result = real_fsync(fd);
    first_crash_point_check("fsync_after");
    return result;
}
```

**Advantages:**
- No source modification required
- Catches all persistence operations automatically
- Can inject crashes before/after each syscall

**Disadvantages:**
- Platform-specific (Linux LD_PRELOAD, macOS DYLD_INSERT_LIBRARIES)
- More complex implementation
- May interact poorly with custom I/O libraries

**Hybrid approach (recommended):**
Use explicit annotations for critical points + syscall interception for comprehensive coverage.

### 6.2 Filesystem State Management

The "state at crash" must be preserved perfectly for deterministic recovery.

#### Copy-on-Write (CoW) Strategy

```
Test Execution Flow with FS State:

Initial State:
┌─────────────┐
│ /tmp/first/ │
│   run_0/    │ ← Empty baseline
└─────────────┘

After some operations:
┌─────────────┐
│ run_0/      │
│   wal.log   │ ← Data written but not synced
│   data.db   │
└─────────────┘

On Crash (at point N):
┌─────────────┐      ┌─────────────┐
│ run_0/      │  →   │ crash_N/    │ ← Snapshot taken
│   wal.log   │      │   wal.log   │   (CoW or copy)
│   data.db   │      │   data.db   │
└─────────────┘      └─────────────┘

Restart:
┌─────────────┐
│ crash_N/    │ ← Recovery runs against this state
│   wal.log   │
│   data.db   │
└─────────────┘
```

#### Implementation Options

**1. Btrfs/ZFS Snapshots (Linux)**
```rust
// Take instant CoW snapshot
std::process::Command::new("btrfs")
    .args(&["subvolume", "snapshot", "run_0", "crash_42"])
    .status()?;
```

**2. Overlayfs (Portable Linux)**
```rust
// Create overlay with upper dir for changes
mount(
    "overlay",
    "crash_42",
    "overlay",
    "lowerdir=baseline,upperdir=run_0,workdir=work"
);
```

**3. Directory Copy (Portable but slower)**
```rust
// Simple recursive copy
fn snapshot_directory(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        
        if file_type.is_dir() {
            snapshot_directory(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}
```

**4. tmpfs with periodic checkpoints (Fast for small tests)**
```rust
// Run entirely in RAM
let test_dir = PathBuf::from("/dev/shm/first_test");
fs::create_dir_all(&test_dir)?;
```

#### Handling Page Cache

**Critical insight:** When simulating power loss, unsynced data in the OS page cache is lost.

```
Application writes data:
  write() → OS page cache → (NOT on disk yet)
                ↓
         If crash here: DATA LOST
                ↓
  fsync() → flush to disk → (Now durable)
                ↓
         If crash here: DATA PERSISTS
```

**FIRST's approach:**
```rust
// Before crashing, we do NOT call sync() at the OS level
// This simulates power loss where page cache is lost

// The SIGKILL ensures:
// 1. No application cleanup
// 2. No implicit flushes
// 3. Only fsync'd data survives
```

**Verification that unsynced data is lost:**
```bash
# After SIGKILL, the kernel may still have pending writes
# To truly simulate power loss, use:
echo 3 > /proc/sys/vm/drop_caches  # Drop page cache (requires root)

# Or use O_DIRECT flag for writes to bypass page cache entirely
```

### 6.3 Process Termination

To simulate a crash, FIRST terminates the process immediately.

#### Termination Mechanism

**Hard kill approach:**
```rust
pub fn trigger_crash() -> ! {
    // Log for reproducibility
    log_crash_point();
    
    // SIGKILL cannot be caught or ignored
    // This is the closest to power loss
    unsafe {
        libc::kill(std::process::id() as i32, libc::SIGKILL);
    }
    
    // Unreachable, but helps compiler
    std::process::exit(137);  // 128 + 9 (SIGKILL)
}
```

**Why SIGKILL?**
- Cannot be caught by signal handlers
- Prevents `Drop` destructors from running
- Prevents standard library cleanup (buffered I/O flush)
- Models power loss accurately

**Alternative for debugging (gentler):**
```rust
pub fn trigger_crash_debug() -> ! {
    // In debug mode, allow core dumps
    unsafe {
        libc::raise(libc::SIGABRT);
    }
    std::process::exit(137);
}
```

#### Preventing Cleanup

```
Normal process exit:
  main() → Drop handlers → atexit hooks → kernel cleanup

SIGKILL exit:
  main() → [KILLED] → kernel cleanup only
           ↑
           No destructors run!
```

**Implications:**
```rust
// This destructor will NOT run on crash
impl Drop for MyDatabase {
    fn drop(&mut self) {
        // [!] This flush won't happen on SIGKILL
        self.flush_buffers();
    }
}

// This is GOOD - it tests real crash behavior
// The database MUST handle recovery without cleanup
```

#### Platform-Specific Gotchas

**1. Memory-mapped files (mmap)**
```rust
// Changes to mmapped files are asynchronous
let mapped = mmap(fd, ...);
mapped[0] = 42;  // Write to memory

// [!] This change may or may not be on disk!

// Must explicitly sync:
msync(mapped, MS_SYNC);  // Now durable

// FIRST treats mmap mutations as volatile until msync()
```

**2. Open file descriptors**
```rust
// FDs are closed by kernel on process death
let file = File::create("test.db")?;
file.write_all(b"data")?;
// Crash here → kernel closes FD
// But data might not be flushed to disk!
```

**3. Shared memory**
```rust
// [!] SysV shared memory persists after process death
let shm_id = shmget(key, size, IPC_CREAT);

// Better: Use POSIX shared memory with unlink
shm_open(...);
shm_unlink(...);  // Auto-cleanup

// Or FIRST provides cleanup hooks
```

**4. Locks and semaphores**
```rust
// File locks are released on process death (good)
flock(fd, LOCK_EX);
// Crash → lock automatically released

// POSIX semaphores may need cleanup
// FIRST isolates tests to avoid leakage
```

### 6.4 Test Isolation

Each test run must be isolated to prevent interference.

```
┌─────────────────────────────────────────┐
│          Test Harness (FIRST)           │
├─────────────────────────────────────────┤
│                                         │
│  ┌────────────┐  ┌────────────┐        │
│  │ Test Run 1 │  │ Test Run 2 │  ...   │
│  ├────────────┤  ├────────────┤        │
│  │ crash_0/   │  │ crash_1/   │        │
│  │  wal.log   │  │  wal.log   │        │
│  │  data.db   │  │  data.db   │        │
│  └────────────┘  └────────────┘        │
│                                         │
│  Isolated:                              │
│  • Separate directories                 │
│  • Separate processes                   │
│  • No shared state                      │
└─────────────────────────────────────────┘
```

**Isolation strategy:**
```rust
impl TestRunner {
    fn run_crash_iteration(&self, crash_point: usize) -> TestResult {
        // 1. Create isolated directory
        let test_dir = format!("/tmp/first/crash_{}", crash_point);
        fs::create_dir_all(&test_dir)?;
        
        // 2. Fork or spawn child process
        match unsafe { fork() } {
            0 => {
                // Child: run test workload
                self.execute_workload(&test_dir, crash_point);
            },
            child_pid => {
                // Parent: wait for crash
                waitpid(child_pid, ...);
                
                // 3. Restart in recovery mode
                self.execute_recovery(&test_dir);
            }
        }
        
        // 4. Cleanup
        fs::remove_dir_all(&test_dir)?;
    }
}
```

---

## 7. State Space and Performance

### 7.1 The Explosion Problem

Crash testing inherently involves exploring a large space of failure scenarios.

**Naive calculation:**
- N operations with M crash points each
- Total crash points = N × M
- Each crash point requires a full test run

**Example:**
```
Workload:
  10 database inserts
  Each insert: 2 writes + 1 fsync = 3 crash points
  
Total crash points: 10 × 3 = 30

If each test takes 50ms (tmpfs, small DB):
  30 runs × 50ms = 1.5 seconds
  
Even with 1,000 operations:
  3,000 runs × 50ms = 2.5 minutes
```

**Conclusion:** Systematic exploration is **totally feasible** for unit/integration testing.

### 7.2 Optimization Strategies

#### 1. Equivalence Pruning

Many crash points are semantically equivalent.

```
Example:
  write("a")
  write("b")   ← No persistence op between
  write("c")   ← these writes
  fsync()

Optimization:
  Crash points after write("a"), write("b"), write("c")
  are equivalent (all lost without fsync).
  
  → Test only: after write("a") and after fsync()
```

**Implementation:**
```rust
struct CrashScheduler {
    last_fsync: usize,
    
    fn should_test_point(&self, point: usize) -> bool {
        // Only test points after each fsync
        point <= self.last_fsync || point % 10 == 0
    }
}
```

#### 2. Heuristic Prioritization

Focus on high-value crash points first.

```
Priority order:
  1. After fsync() - durability boundary
  2. Between multi-step operations (write → rename)
  3. Explicit developer annotations
  4. Random sampling of remaining points
```

#### 3. Parallel Execution

Crash points are independent - run in parallel.

```
┌──────────┐  ┌──────────┐  ┌──────────┐
│ Crash 0  │  │ Crash 1  │  │ Crash 2  │
│ (core 0) │  │ (core 1) │  │ (core 2) │
└──────────┘  └──────────┘  └──────────┘
     ↓              ↓              ↓
  Results      Results        Results
```

**Performance:**
```
Sequential: 1000 points × 50ms = 50 seconds
Parallel (8 cores): 50s / 8 = 6.25 seconds
```

### 7.3 Performance Characteristics

```
┌────────────────────────────────────────────────┐
│          FIRST Performance Profile             │
├────────────────────────────────────────────────┤
│                                                │
│  Crash Points vs. Test Time                   │
│                                                │
│  Time                                          │
│   │                                          │
│   │                            •••           │
│   │                       •••                │
│   │                  •••                     │
│   │             •••                          │
│   │        •••                               │
│   │   •••                                    │
│   └───────────────────────────────          │
│       0   100  200  300  400  500            │
│            Crash Points                      │
│                                                │
│  Linear scaling: O(N) where N = crash points  │
│  Per-point overhead: ~50ms (tmpfs)            │
│  Parallel speedup: Near-linear up to 8 cores  │
└────────────────────────────────────────────────┘
```

### 7.4 Real-World Examples

**Simple append-only log:**
- Operations: 100 appends + syncs
- Crash points: ~300
- Test time: 15 seconds (sequential), 2 seconds (8-core)

**LSM tree with compaction:**
- Operations: 1000 inserts, 5 compactions
- Crash points: ~500 (with pruning)
- Test time: 25 seconds (sequential), 4 seconds (8-core)

**B-tree with node splits:**
- Operations: 500 inserts (10 splits)
- Crash points: ~200 (focusing on split boundaries)
- Test time: 10 seconds (sequential), 2 seconds (8-core)

---

## 8. Invariants and Validation

FIRST evaluates crash correctness using **invariants** - properties that must hold after recovery.

### 8.1 Invariant Philosophy

**Invariants describe truths, not expected outputs.**

```rust
// [BAD] Testing specific outcomes
assert_eq!(db.get("key"), Some("value"));  
// What if the crash happened before the write?

// [GOOD] Testing properties
assert!(db.is_internally_consistent());
assert!(all_committed_data_present() || db.is_empty());
```

### 8.2 Common Storage Invariants

#### 1. Atomicity Invariant
```rust
fn verify_transaction_atomicity(db: &Database) {
    let tx_log = db.get_transaction_log();
    
    for tx in tx_log {
        let committed = db.is_committed(tx.id);
        let writes = db.get_writes(tx.id);
        
        if committed {
            // All writes must be present
            assert_eq!(writes.len(), tx.expected_writes);
        } else {
            // No writes should be visible
            assert_eq!(writes.len(), 0);
        }
    }
}
```

#### 2. Durability Invariant
```rust
fn verify_durability(db: &Database) {
    // Data marked as "synced" must be present
    let synced_keys = db.read_sync_marker();
    
    for key in synced_keys {
        assert!(
            db.contains(&key),
            "Key {:?} was marked synced but is missing!",
            key
        );
    }
}
```

#### 3. Integrity Invariant
```rust
fn verify_integrity(db: &Database) {
    // Check checksums
    for page in db.pages() {
        let computed = page.compute_checksum();
        let stored = page.checksum;
        assert_eq!(
            computed, stored,
            "Checksum mismatch on page {}", page.id
        );
    }
    
    // Check internal structure
    for node in db.btree_nodes() {
        assert!(node.keys_sorted(), "Keys not sorted in node {}", node.id);
        assert!(node.pointers_valid(), "Invalid pointer in node {}", node.id);
    }
}
```

#### 4. Monotonicity Invariant
```rust
fn verify_monotonicity(wal: &WriteAheadLog) {
    let entries = wal.read_all();
    
    for window in entries.windows(2) {
        assert!(
            window[1].sequence_number >= window[0].sequence_number,
            "Sequence numbers decreased: {} -> {}",
            window[0].sequence_number,
            window[1].sequence_number
        );
    }
}
```

#### 5. Consistency Invariant
```rust
fn verify_manifest_consistency(db: &Database) {
    let manifest_files = db.manifest_list_sstables();
    let actual_files = db.scan_sstable_directory();
    
    // No orphaned files
    for file in &actual_files {
        assert!(
            manifest_files.contains(file),
            "Orphaned SSTable: {:?}", file
        );
    }
    
    // No dangling references
    for file in &manifest_files {
        assert!(
            actual_files.contains(file),
            "Manifest references missing file: {:?}", file
        );
    }
}
```

### 8.3 Validation Timing

```
Test Execution Timeline:

Normal Execution:
  ┌───────────────────────────────┐
  │ Application logic runs        │
  │ NO invariant checks here      │
  └───────────────────────────────┘
               │
               ▼ Crash injected
  ┌───────────────────────────────┐
  │ Process killed                │
  └───────────────────────────────┘
               │
               ▼ Restart
  ┌───────────────────────────────┐
  │ Recovery logic runs           │
  │ (WAL replay, index rebuild)   │
  └───────────────────────────────┘
               │
               ▼ After recovery complete
  ┌───────────────────────────────┐
  │ INVARIANT CHECKS RUN HERE     │ ← FIRST calls verify()
  │ Must all pass                 │
  └───────────────────────────────┘
```

### 8.4 Failure Reporting

When an invariant is violated:

```
FIRST Failure Report:
════════════════════════════════════════════════
Test: test_append_log
Crash Point: 12 ("after_entry1")
Schedule Seed: 42
Filesystem State: /tmp/first/crash_12/

Invariant Violation:
  File: tests/append_log.rs:45
  Invariant: "Checksum must match"
  
  Expected checksum: 0x1a2b3c4d
  Actual checksum:   0x00000000
  
  Affected file: mylog.wal
  Offset: 1024
  
Reproduction:
  $ FIRST_SEED=42 FIRST_CRASH_POINT=12 cargo test test_append_log
  
Filesystem contents preserved at:
  /tmp/first/crash_12/
════════════════════════════════════════════════
```

### 8.5 Responsibility Boundaries

```
┌─────────────────────────────────────────┐
│           FIRST Responsibilities         │
├─────────────────────────────────────────┤
│ • Execute invariant checks at correct   │
│   time (after recovery)                 │
│ • Ensure crashes are deterministic      │
│ • Report failures with context          │
│ • Preserve FS state for inspection      │
└─────────────────────────────────────────┘
                  ↕
┌─────────────────────────────────────────┐
│    System Under Test Responsibilities   │
├─────────────────────────────────────────┤
│ • Define meaningful invariants          │
│ • Implement correct recovery logic      │
│ • Interpret and fix failures            │
│ • Ensure data structures are checkable  │
└─────────────────────────────────────────┘
```

---

## 9. Concurrency Model

Concurrency significantly complicates crash reasoning and state exploration.

### 9.1 Initial Scope: Single Logical Thread

In its initial design, FIRST constrains concurrency to keep crash behavior deterministic and debuggable.

**Assumptions:**
- A single logical execution thread interacting with persistent state
- Serialized persistence operations
- No concurrent mutation of on-disk state during a single execution

```
Single-Threaded Model:

  Thread 1 (only):
    │
    ├─ write("a")     ← Crash point 0
    ├─ fsync()        ← Crash point 1
    ├─ write("b")     ← Crash point 2
    ├─ fsync()        ← Crash point 3
    │
    ▼

  Crash points are well-defined and linear
```

**Rationale:**
- Many crash bugs manifest even under serialized execution
- Deterministic replay is straightforward
- State space remains finite and explorable
- Focus on WAL, metadata, recovery logic bugs

### 9.2 Handling Background Threads

Storage engines often use background threads (compaction, flushing, checkpointing). How can FIRST test these?

#### Option 1: Serialization Mode

The storage engine provides a "test mode" that serializes background work.

```rust
let db = Database::open(path)
    .with_background_threads(0)  // Disable async work
    .with_explicit_flush(true);  // Manual control

// Now operates in single-threaded mode for testing
db.put("key", "value");
db.manual_flush();  // Explicitly trigger flush
```

#### Option 2: Cooperative Interception

Background threads register with FIRST to serialize their persistence operations.

```rust
// In the storage engine's background thread:
fn background_compaction() {
    loop {
        let work = get_next_compaction();
        
        // Serialize with FIRST scheduler
        first::block_on_op("compaction_write", || {
            write_compacted_data(work);
        });
        
        first::block_on_op("compaction_sync", || {
            fsync_compacted_data();
        });
    }
}
```

**FIRST's scheduler:**
```rust
pub fn block_on_op<F, R>(label: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    // Wait for our turn in the global operation order
    SCHEDULER.wait_for_turn(label);
    
    // Execute the operation
    let result = f();
    
    // Check if crash point reached
    crash_point(label);
    
    // Allow next operation to proceed
    SCHEDULER.release_turn();
    
    result
}
```

**Concurrency model with cooperative interception:**

```
Main Thread:            Background Thread:
    │                        │
    ├─ put("a")              │
    ├─ fsync() ────────┐     │
    │                  │     │
    │              [BARRIER] │
    │                  │     ├─ block_on_op("compact")
    │                  └────>│
    │                        ├─ write_data()
    │                        ├─ fsync()
    │                        └──┐
    ├─ put("b") <──────────────┘
    │
    ▼
```

### 9.3 Future Concurrency Support

Potential extensions (not in initial version):

#### 1. Controlled Interleavings
```rust
first::test()
    .with_threads(2)
    .interleaving_strategy(InterleavingStrategy::Systematic)
    .run(|env| {
        // Thread 1
        spawn(|| db.put("a", "1"));
        
        // Thread 2  
        spawn(|| db.put("b", "2"));
    });
```

#### 2. Happens-Before Constraints
```rust
first::test()
    .run(|env| {
        let op1 = async { db.write("a") };
        let op2 = async { db.write("b") };
        
        // Test both orders
        first::explore_orderings([op1, op2]);
    });
```

#### 3. Deterministic Thread Scheduling
```rust
// Use deterministic scheduler (like FoundationDB's simulation)
first::test()
    .deterministic_scheduler(seed)
    .run(|env| {
        // Threads scheduled deterministically
    });
```

**These extensions will only be added if they preserve:**
- Determinism
- Reproducibility  
- Debuggability

---

## 10. Comparison to Existing Work

FIRST fills a specific gap in the crash testing ecosystem.

### 10.1 Comparison Table

| Feature | **FIRST** | CrashMonkey | Jepsen | dm-flakey | FoundationDB Sim |
|---------|----------|-------------|--------|-----------|------------------|
| **Target** | Storage Engines (Local) | Filesystems | Distributed Systems | Block Devices | Full System |
| **Orchestration** | Unit Test Library | Kernel Module / VM | Network Partition | Kernel Module | Internal Tool |
| **Deterministic?** | [YES] | [MOSTLY] | [NO] | [YES] (scriptable) | [YES] |
| **Knowledge Level** | App-aware (Crash points) | FS-aware (bio layer) | Black box | Block-level only | Full system |
| **Ease of Use** | `cargo test` | Requires VM setup | Requires cluster | Root/Kernel setup | Not extractable |
| **Recovery Testing** | [YES] Built-in | [YES] | [NO] | [LIMITED] | [YES] |
| **Language Support** | Rust (+ planned C++/Go) | Any (VM-based) | Any (network-based) | Any (kernel) | Internal only |
| **Open Source?** | [YES] (planned) | [YES] | [YES] | [YES] | [NO] |
| **Typical Test Time** | Seconds to minutes | Minutes to hours | Hours | Minutes | N/A |
| **State Exploration** | Systematic (bounded) | Random | Random | Scriptable | Systematic |

### 10.2 Positioning

```
┌────────────────────────────────────────────────────────┐
│                  Testing Landscape                     │
├────────────────────────────────────────────────────────┤
│                                                        │
│  Hardware Level:                                       │
│  └─ dm-flakey, fail_make_request                      │
│      [Block device fault injection]                   │
│                                                        │
│  Filesystem Level:                                     │
│  └─ CrashMonkey                                        │
│      [Filesystem crash consistency]                   │
│                                                        │
│  Storage Engine Level: ← FIRST OPERATES HERE          │
│  └─ FIRST                                              │
│      [WAL, LSM, B-tree crash testing]                 │
│                                                        │
│  Application Level:                                    │
│  └─ Unit tests, integration tests                     │
│      [Application logic testing]                      │
│                                                        │
│  Distributed Level:                                    │
│  └─ Jepsen                                             │
│      [Network partitions, consensus]                  │
│                                                        │
└────────────────────────────────────────────────────────┘
```

### 10.3 Why Not Use X?

**Q: Why not use CrashMonkey?**

A: CrashMonkey tests filesystem implementations, not storage engines. It operates at the block I/O layer and doesn't understand application-level persistence boundaries (e.g., "after WAL commit but before manifest update"). FIRST is application-aware.

**Q: Why not use Jepsen?**

A: Jepsen tests distributed systems by injecting network faults and clock skew. FIRST tests local persistence and crash recovery. Different problem domains.

**Q: Why not use dm-flakey?**

A: dm-flakey injects faults at the block device level, requiring kernel modules and root access. FIRST operates at syscall boundaries and runs as a regular user process. It's also application-aware, not block-aware.

**Q: Why not use FoundationDB's simulation testing?**

A: FoundationDB's simulator is:
1. Proprietary and deeply integrated into their codebase
2. Not available as a reusable framework
3. Requires rewriting systems in a specific style

FIRST is designed to be a reusable, open-source framework that works with existing storage engines.

**Q: Why not just use fuzzing?**

A: Fuzzing generates random inputs to find crashes. FIRST generates systematic crash scenarios to find recovery bugs. Complementary approaches—FIRST focuses on "what happens when power is lost" rather than "what inputs cause crashes."

### 10.4 Academic Prior Art

FIRST builds on research in crash consistency testing:

- **ALICE (OSDI '14)**: Logical crash consistency checking for filesystems
- **CrashMonkey (OSDI '18)**: Automated crash consistency testing
- **B3 (EuroSys '19)**: Application-level crash testing for Android apps
- **AGAMOTTO (SOSP '21)**: Filesystem testing via record/replay

**FIRST's contribution:** A practical, reusable framework specifically for storage engine developers, with a focus on determinism and ease of use.

---

## 11. Limitations and Future Work

FIRST is intentionally scoped to remain focused, deterministic, and practical.

### 11.1 Current Limitations

#### 1. Single-Process Focus
```
[SUPPORTED] Single-node storage engines
[NOT SUPPORTED] Distributed consensus, replication
```

**Mitigation:** For distributed systems, use Jepsen or similar tools. FIRST focuses on getting local persistence right first.

#### 2. No Hardware Fault Simulation
```
[SUPPORTED] Power loss (clean crash)
[NOT SUPPORTED] Bit rot, sector corruption, disk failures
```

**Rationale:** These are rare compared to crash consistency bugs. Tools like dm-flakey can complement FIRST for hardware fault testing.

#### 3. Concurrency Support Limited
```
[SUPPORTED] Serialized persistence paths
[PARTIAL] Background threads with cooperative hooks
[NOT SUPPORTED] Arbitrary concurrent interleavings (initially)
```

**Mitigation:** Many bugs appear even under serialized execution. Future versions may add controlled concurrency.

#### 4. Filesystem-Specific Semantics
```
[TESTED] ext4, btrfs, XFS, tmpfs (Linux)
[UNTESTED] macOS APFS, Windows NTFS (semantics may differ)
```

**Mitigation:** Document tested filesystems. Future work may abstract filesystem semantics.

#### 5. No Distributed Operations
```
[NOT SUPPORTED] Network partitions, leader election, clock skew
```

**Rationale:** Out of scope. Use Jepsen for distributed testing.

### 11.2 Known Edge Cases

#### mmap Without Explicit Sync
```rust
let mapped = mmap(fd, ...);
mapped[0] = 42;  // [WARNING] Timing of persistence is undefined

// Must call:
msync(mapped, MS_SYNC);  // Now explicitly durable
```

FIRST treats non-msynced mmap writes as volatile.

#### Asynchronous I/O (io_uring, libaio)
```rust
// Completion timing is asynchronous
io_uring_submit(...);

// [WARNING] When does data actually reach disk?
```

**Current approach:** Require explicit barriers (fsync) for determinism.

#### Partial Sector Writes (Torn Writes)
```
Physical disk:
  [Sector 1: Half old, half new data]  ← Torn write

FIRST does NOT model this (yet)
```

**Mitigation:** Storage engines should use checksums to detect corruption.

### 11.3 Future Work

#### Phase 1 Extensions (Near-term)
- [ ] Syscall interception mode (LD_PRELOAD)
- [ ] Crash point pruning heuristics
- [ ] Parallel crash point exploration
- [ ] Better diagnostics and debugging output

#### Phase 2 Extensions (Medium-term)
- [ ] C/C++ language bindings
- [ ] Go language bindings
- [ ] Support for Windows and macOS
- [ ] Integration with existing test frameworks (gtest, pytest)

#### Phase 3 Extensions (Long-term)
- [ ] Controlled concurrent interleavings
- [ ] Deterministic multi-threaded scheduling
- [ ] Hardware fault injection (torn writes, bit flips)
- [ ] Distributed crash testing (multi-node)

### 11.4 What FIRST Will Never Do

To maintain focus and simplicity:

**[NOT A GOAL] Will not become a fuzzer**
- Fuzzing is orthogonal to crash testing
- Fuzzers generate inputs; FIRST generates crash scenarios

**[NOT A GOAL] Will not guarantee application correctness**
- FIRST provides testing infrastructure
- Correctness depends on invariants defined by the developer

**[NOT A GOAL] Will not replace formal verification**
- FIRST is empirical testing, not proof
- Formal methods are complementary

**[NOT A GOAL] Will not handle Byzantine failures**
- Malicious behavior is out of scope
- Focus is on crash/power-loss scenarios

---

## Appendix: Quick Reference

### A.1 Common Patterns

**Basic test structure:**
```rust
first::test()
    .run(|env| {
        // Workload
    })
    .verify(|env, info| {
        // Invariants
    });
```

**With explicit crash points:**
```rust
operation();
first::crash_point("label");
```

**With configuration:**
```rust
first::test()
    .seed(12345)
    .max_crashes(100)
    .run(...)
    .verify(...);
```

### A.2 Environment API

```rust
env.path(\"name\")             // Path inside workspace for persistent state
env.crash_point_count()   // Number of discovered crash points
```

### A.3 Crash Info

```rust
crash_info.point_id       // Numeric ID of crash point
crash_info.label          // Optional label (if explicit)
crash_info.operation      // Syscall that was interrupted
```

### A.4 Common Invariants

```rust
// Atomicity
assert!(all_or_nothing(tx));

// Durability  
assert!(synced_data_exists());

// Integrity
assert!(checksums_valid());

// Monotonicity
assert!(sequence_increasing());

// Consistency
assert!(no_orphaned_files());
```

---

## Conclusion

FIRST provides a **deterministic, systematic framework** for testing crash consistency in storage engines.

**Key principles:**

1. **Crashes as first-class citizens** - Not rare events, but expected scenarios
2. **Invariants over outputs** - Properties that must always hold
3. **Determinism** - Every failure is reproducible
4. **Focused scope** - Local persistence, not distributed systems
5. **Practical** - Fast enough for continuous integration

**What makes FIRST different:**

- Application-aware crash points (not block-level or random)
- Built-in recovery testing (every crash → restart → validate)
- Designed for storage engine developers (not kernel hackers)
- Reusable across projects (not bespoke infrastructure)

By making crash consistency **testable, repeatable, and debuggable**, FIRST helps storage systems achieve the same level of rigor for recovery paths as they do for normal execution.

---

**Status:** Early development phase  
**Feedback:** Welcome from storage and database engineers  
**Focus Areas:** Crash boundaries, invariant modeling, recovery semantics  
**Repository:** [To be announced]  
**License:** [To be announced]