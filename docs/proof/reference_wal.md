# Reference WAL: Proof of Correctness

FIRST deterministically exposed an atomicity bug in the reference WAL.

## The Bug

```rust
// BUGGY: COMMIT written before records are durable
writeln!(self.file, "COMMIT {}", txid)?;
crash_point("after_commit_write");
self.file.sync_all()?;  // ← too late
```

If crash occurs after COMMIT but before `fsync()`, records may not reach disk, but COMMIT does → **atomicity violation**.

## Why It's Subtle

- Invisible without crashes (all writes succeed)
- Page cache hides the non-atomicity
- Single `fsync()` *looks* correct
- Common mistake in WAL implementations

## FIRST Output

```
[first] crash point 5: FAILED (see /tmp/first/run_5)
[first] crash label: "after_commit_write"

Atomicity violation: committed transaction has only 1/3 records visible.
```

## The Fix

```diff
  pub fn commit(&mut self, txid: TxId) {
+     self.file.sync_all()?;  // ← ensure records durable FIRST
      writeln!(self.file, "COMMIT {}", txid)?;
      self.file.sync_all()?;
```

## Verification

After fix:

```
[first] all 7 crash points passed
```

## Reproduce

```bash
cd examples/reference_wal
cargo test transaction_atomicity_under_crash -- --nocapture
```
