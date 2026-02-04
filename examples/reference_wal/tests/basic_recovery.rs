//! Basic recovery tests for the reference WAL.
//!
//! These tests verify clean-restart recovery semantics.

use reference_wal::Wal;
use tempfile::tempdir;

/// Test that committed data survives a clean restart.
#[test]
fn committed_data_survives_restart() {
    let dir = tempdir().unwrap();

    // First session: write and commit
    {
        let mut wal = Wal::open(dir.path()).unwrap();
        let tx = wal.begin();
        wal.put(tx, "key1", "value1");
        wal.put(tx, "key2", "value2");
        wal.commit(tx);
    }

    // Second session: verify recovery
    let wal = Wal::open(dir.path()).unwrap();
    assert_eq!(wal.get("key1"), Some("value1"));
    assert_eq!(wal.get("key2"), Some("value2"));
}

/// Test that uncommitted data is not visible after restart.
#[test]
fn uncommitted_data_not_visible_after_restart() {
    let dir = tempdir().unwrap();

    // First session: write but do NOT commit
    {
        let mut wal = Wal::open(dir.path()).unwrap();
        let tx = wal.begin();
        wal.put(tx, "key", "value");
        // No commit - drop WAL
    }

    // Second session: verify uncommitted data is absent
    let wal = Wal::open(dir.path()).unwrap();
    assert_eq!(wal.get("key"), None);
}
