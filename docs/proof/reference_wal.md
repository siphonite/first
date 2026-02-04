# Reference WAL: FIRST Proof of Correctness

## Summary

FIRST deterministically exposed and verified the fix for an atomicity bug in the reference WAL where transactions could be partially visible after crash recovery.

---

## The Bug

The WAL wrote the `COMMIT` marker before ensuring all transaction records were durable:

```rust
// BUGGY: records may not be on disk when COMMIT is written
writeln!(self.file, "COMMIT {}", txid)?;
crash_point("after_commit_write");
self.file.sync_all()?;  // ← too late
```

If a crash occurred after writing COMMIT but before `fsync()`, the kernel could flush COMMIT to disk before all PUT records, resulting in a committed transaction with missing records.

---

## Why It's Subtle

- **Invisible in normal testing**: Without crashes, all writes complete successfully
- **Page cache hides the problem**: Writes appear atomic from the application's perspective
- **Correct-looking code**: A single `fsync()` after COMMIT *seems* sufficient
- **Non-deterministic in production**: Depends on kernel flush timing
- **Common pattern**: Many WAL implementations make this exact mistake

---

## Crash Point That Exposed It

```
after_commit_write
```

This crash point fires after the COMMIT record is written to the page cache but before any `fsync()` call.

---

## FIRST Output (Failing Run)

Before the fix, FIRST reported:

```
[first] crash point 1: OK
[first] crash point 2: OK
[first] crash point 3: OK
[first] crash point 4: OK
[first] crash point 5: FAILED (see /tmp/first/run_5)
[first] crash label: "after_commit_write"
[first] reason: verification failed with exit code 101
[first] to reproduce:
  FIRST_PHASE=VERIFY FIRST_CRASH_TARGET=5 FIRST_WORK_DIR=/tmp/first/run_5 \
  FIRST_CRASH_POINT_ID=5 FIRST_CRASH_LABEL="after_commit_write" \
  cargo test transaction_atomicity_under_crash -- --exact
```

The verification phase panicked with:

```
Atomicity violation at crash point 'after_commit_write': 
committed transaction has only 1/3 records visible. 
key1=Some("value1"), key2=None, key3=None
```

---

## The Fix

Add `fsync()` before writing COMMIT to ensure records are durable first:

```diff
     pub fn commit(&mut self, txid: TxId) {
+        self.file.sync_all().expect("failed to fsync records");
         writeln!(self.file, "COMMIT {}", txid).expect("failed to write COMMIT");
         crash_point("after_commit_write");
         self.file.sync_all().expect("failed to fsync after COMMIT");
```

Now the ordering is:

1. Write all PUT records
2. `fsync()` ← records durable
3. Write COMMIT
4. `fsync()` ← COMMIT durable

A crash at any point now guarantees atomicity:
- Before step 2: no records, no COMMIT
- After step 2, before step 4: records durable, no COMMIT visible
- After step 4: records and COMMIT both durable

---

## Verification

After the fix, FIRST reports all crash points passed:

```
[first] crash point 1: OK
[first] crash point 2: OK
[first] crash point 3: OK
[first] crash point 4: OK
[first] crash point 5: OK
[first] crash point 6: OK
[first] crash point 7: OK
[first] all 7 crash points passed
```

---

## How to Reproduce the Original Bug

1. Check out the commit immediately before the fix (see `Fixes #9` in git history)
2. Run the crash consistency test:

```bash
cd examples/reference_wal
cargo test transaction_atomicity_under_crash -- --nocapture
```

3. Observe the failure at `after_commit_write`

To re-run just the failing verification:

```bash
FIRST_PHASE=VERIFY \
FIRST_CRASH_TARGET=5 \
FIRST_WORK_DIR=/tmp/first/run_5 \
FIRST_CRASH_POINT_ID=5 \
FIRST_CRASH_LABEL="after_commit_write" \
cargo test transaction_atomicity_under_crash -- --exact --nocapture
```

---

## Conclusion

FIRST successfully:

1. **Found** a real atomicity bug via systematic crash point exploration
2. **Reproduced** the bug deterministically with a single command
3. **Verified** the fix by confirming all crash points pass

This demonstrates FIRST's effectiveness for crash-consistency testing of storage engines.
