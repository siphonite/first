//! Crash consistency tests for the reference WAL using FIRST.
//!
//! These tests verify that transaction atomicity is preserved under
//! SIGKILL-based crashes at any crash point.

use reference_wal::Wal;

/// Test that transaction atomicity is preserved under crashes.
///
/// Invariant: After recovery, either ALL records of a committed transaction
/// are visible, or NONE are visible. Partial visibility violates atomicity.
#[test]
fn transaction_atomicity_under_crash() {
    first::test()
        .run(|env| {
            let mut wal = Wal::open(&env.path("wal")).unwrap();

            // Begin a transaction with multiple records
            let tx = wal.begin();
            wal.put(tx, "key1", "value1");
            wal.put(tx, "key2", "value2");
            wal.put(tx, "key3", "value3");
            wal.commit(tx);
        })
        .verify(|env, crash_info| {
            // Recovery: reopen the WAL (triggers recovery logic)
            let wal = Wal::open(&env.path("wal")).unwrap();


            // Check visibility of each record
            let key1 = wal.get("key1");
            let key2 = wal.get("key2");
            let key3 = wal.get("key3");

            // Count how many records are visible
            let visible_count = [key1.is_some(), key2.is_some(), key3.is_some()]
                .iter()
                .filter(|&&v| v)
                .count();

            // Atomicity invariant: all-or-nothing
            match visible_count {
                0 => {
                    // Transaction not committed - acceptable
                }
                3 => {
                    // All records visible - verify values are correct
                    assert_eq!(key1, Some("value1"), "key1 has wrong value");
                    assert_eq!(key2, Some("value2"), "key2 has wrong value");
                    assert_eq!(key3, Some("value3"), "key3 has wrong value");
                }
                partial => {
                    // ATOMICITY VIOLATION: committed transaction has missing records
                    panic!(
                        "Atomicity violation at crash point '{}': \
                         committed transaction has only {}/3 records visible. \
                         key1={:?}, key2={:?}, key3={:?}",
                        crash_info.label,
                        partial,
                        key1,
                        key2,
                        key3
                    );
                }
            }
        })
        .execute();
}
