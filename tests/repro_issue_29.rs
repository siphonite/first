//! Reproduction test for Issue #29: Work directory collisions.
//!
//! Two tests running in parallel should collide on `/tmp/first/run_1`
//! if the fix is not applied.

use std::fs::File;
use std::io::Write;
use std::thread;
use std::time::Duration;

#[test]
fn collision_test_a() {
    first::test()
        .run(|env| {
            let path = env.path("collision.txt");

            // Write unique content
            let mut file = File::create(&path).unwrap();
            file.write_all(b"TEST_A").unwrap();

            // Sleep to increase chance of overlap
            thread::sleep(Duration::from_millis(100));

            first::crash_point("point_a");
        })
        .verify(|env, _| {
            let path = env.path("collision.txt");
            let contents = std::fs::read_to_string(&path).unwrap_or_default();

            // If we collided, we might see "TEST_B" or mixed content
            assert_eq!(
                contents, "TEST_A",
                "Content mismatch! Likely collision with concurrent test."
            );
        })
        .execute();
}

#[test]
fn collision_test_b() {
    first::test()
        .run(|env| {
            let path = env.path("collision.txt");

            // Write unique content
            let mut file = File::create(&path).unwrap();
            file.write_all(b"TEST_B").unwrap();

            // Sleep to increase chance of overlap
            thread::sleep(Duration::from_millis(100));

            first::crash_point("point_b");
        })
        .verify(|env, _| {
            let path = env.path("collision.txt");
            let contents = std::fs::read_to_string(&path).unwrap_or_default();

            // If we collided, we might see "TEST_A" or mixed content
            assert_eq!(
                contents, "TEST_B",
                "Content mismatch! Likely collision with concurrent test."
            );
        })
        .execute();
}
