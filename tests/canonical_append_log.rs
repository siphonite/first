//! Canonical example: Append-only log with crash-consistency verification.
//!
//! This test validates the FIRST API by performing a simple append-log
//! workload with explicit crash points and invariant checking.

use std::fs::File;
use std::io::{Read, Write};

#[test]
fn append_log_atomicity() {
    first::test()
        .run(|env| {
            let path = env.path("append.log");

            // Write record 1
            let mut file = File::create(&path).unwrap();
            file.write_all(b"RECORD1\n").unwrap();
            first::crash_point("after_write_1");

            // Write record 2
            file.write_all(b"RECORD2\n").unwrap();
            first::crash_point("after_write_2");

            // Fsync to make durable
            file.sync_all().unwrap();
            first::crash_point("after_fsync");
        })
        .verify(|env, crash_info| {
            let path = env.path("append.log");

            // Read whatever survived
            let mut contents = String::new();
            if let Ok(mut f) = File::open(&path) {
                f.read_to_string(&mut contents).ok();
            }

            let records: Vec<_> = contents.lines().collect();

            // INVARIANT: Records are prefix-consistent
            // Either: [], ["RECORD1"], or ["RECORD1", "RECORD2"]
            // Never: ["RECORD2"] alone (would violate append-only semantics)

            match records.as_slice() {
                [] => { /* Nothing persisted - fine */ }
                ["RECORD1"] => { /* Partial - fine */ }
                ["RECORD1", "RECORD2"] => { /* Complete - fine */ }
                other => {
                    panic!(
                        "Invariant violation at '{}': unexpected log state {:?}",
                        crash_info.label, other
                    );
                }
            }
        })
        .execute();
}
