//! Reference WAL: minimal append-only write-ahead log.

mod wal;

pub use wal::{TxId, Wal};
