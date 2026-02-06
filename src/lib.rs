//! FIRST: Deterministic crash testing framework for storage engines.
//!
//! This crate provides primitives for injecting crashes at specific points
//! in your storage system's execution to verify crash consistency.
//!
//! # Example
//!
//! ```ignore
//! use first::{test, crash_point};
//!
//! fn my_test() {
//!     first::test()
//!         .run(|env| {
//!             let wal_path = env.path("wal");
//!             write_to_wal(&wal_path);
//!             crash_point("after_wal_write");
//!             fsync_wal(&wal_path);
//!             crash_point("after_wal_sync");
//!         })
//!         .verify(|env, crash_info| {
//!             let wal_path = env.path("wal");
//!             let recovered = open_and_recover(&wal_path);
//!             assert!(recovered.is_consistent());
//!         });
//! }
//! ```

mod env;
mod orchestrator;
mod rt;
mod test;

pub use env::{CrashInfo, Env};
pub use rt::crash_point;
pub use test::test;
