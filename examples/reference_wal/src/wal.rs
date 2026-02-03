//! WAL implementation.

use std::path::Path;

/// Transaction identifier.
pub type TxId = u64;

/// Minimal append-only write-ahead log.
pub struct Wal {
    _private: (),
}

impl Wal {
    /// Open or create a WAL at the given path.
    pub fn open(_path: &Path) -> std::io::Result<Self> {
        todo!()
    }

    /// Begin a new transaction.
    pub fn begin(&mut self) -> TxId {
        todo!()
    }

    /// Write a key-value pair within a transaction.
    pub fn put(&mut self, _txid: TxId, _key: &str, _value: &str) {
        todo!()
    }

    /// Commit a transaction.
    pub fn commit(&mut self, _txid: TxId) {
        todo!()
    }

    /// Get a value by key.
    pub fn get(&self, _key: &str) -> Option<&str> {
        todo!()
    }
}
