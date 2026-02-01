//! FIRST: Deterministic crash testing framework for storage engines.
//!
//! This crate provides primitives for injecting crashes at specific points
//! in your storage system's execution to verify crash consistency.
//!
//! # Example
//!
//! ```ignore
//! use first::crash_point;
//!
//! fn my_storage_operation() {
//!     write_to_wal();
//!     crash_point("after_wal_write");
//!     fsync_wal();
//!     crash_point("after_wal_sync");
//! }
//! ```

mod rt;

pub use rt::crash_point;
