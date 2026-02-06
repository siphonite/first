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

            // INVARIANT: Records are prefix-consistent, with durability enforcement
            //
            // Before fsync: any prefix is acceptable ([], ["RECORD1"], or both)
            // After fsync: both records MUST be present (fsync guarantees durability)

            match (crash_info.label.as_str(), records.as_slice()) {
                // After write_1: nothing or RECORD1 is fine
                ("after_write_1", []) => {}
                ("after_write_1", ["RECORD1"]) => {}

                // After write_2: nothing, RECORD1, or both is fine (not yet synced)
                ("after_write_2", []) => {}
                ("after_write_2", ["RECORD1"]) => {}
                ("after_write_2", ["RECORD1", "RECORD2"]) => {}

                // After fsync: MUST have both records (durability guarantee)
                ("after_fsync", ["RECORD1", "RECORD2"]) => {}

                // Any other state is a violation
                (label, state) => {
                    panic!(
                        "Invariant violation at '{}': unexpected log state {:?}",
                        label, state
                    );
                }
            }
        })
        .execute();
}
