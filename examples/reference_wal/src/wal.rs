//! WAL implementation.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use first::crash_point;

/// Transaction identifier.
pub type TxId = u64;

/// Minimal append-only write-ahead log.
pub struct Wal {
    /// Path to the WAL directory.
    dir: PathBuf,
    /// Handle to the WAL file.
    file: File,
    /// In-memory state from committed transactions.
    state: HashMap<String, String>,
    /// Next transaction ID to allocate.
    next_txid: TxId,
}

impl Wal {
    /// Open or create a WAL at the given path.
    ///
    /// Creates the directory if it does not exist.
    /// Recovers state from existing WAL file.
    pub fn open(path: &Path) -> io::Result<Self> {
        // Create directory if needed
        fs::create_dir_all(path)?;

        let wal_path = path.join("wal.log");

        // Recover state from existing WAL
        let (state, max_txid) = if wal_path.exists() {
            Self::recover_from_file(&wal_path)?
        } else {
            (HashMap::new(), 0)
        };

        // Open file in append mode
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&wal_path)?;

        Ok(Self {
            dir: path.to_path_buf(),
            file,
            state,
            next_txid: max_txid + 1,
        })
    }

    /// Recover state from WAL file.
    ///
    /// Returns the recovered state and the maximum transaction ID seen.
    fn recover_from_file(wal_path: &Path) -> io::Result<(HashMap<String, String>, TxId)> {
        let file = File::open(wal_path)?;
        let reader = BufReader::new(file);

        // Pending transactions: txid -> Vec<(key, value)>
        let mut pending: HashMap<TxId, Vec<(String, String)>> = HashMap::new();
        // Committed state
        let mut state: HashMap<String, String> = HashMap::new();
        // Track max txid seen
        let mut max_txid: TxId = 0;

        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            if parts.is_empty() {
                panic!("malformed WAL record: empty line");
            }

            match parts[0] {
                "BEGIN" => {
                    if parts.len() != 2 {
                        panic!("malformed BEGIN record: {}", line);
                    }
                    let txid: TxId = parts[1].parse().expect("invalid txid in BEGIN");
                    if txid > max_txid {
                        max_txid = txid;
                    }
                    pending.insert(txid, Vec::new());
                }
                "PUT" => {
                    if parts.len() != 4 {
                        panic!("malformed PUT record: {}", line);
                    }
                    let txid: TxId = parts[1].parse().expect("invalid txid in PUT");
                    let key = parts[2].to_string();
                    let value = parts[3].to_string();
                    if let Some(ops) = pending.get_mut(&txid) {
                        ops.push((key, value));
                    }
                    // Ignore PUT for unknown txid (transaction not started)
                }
                "COMMIT" => {
                    if parts.len() != 2 {
                        panic!("malformed COMMIT record: {}", line);
                    }
                    let txid: TxId = parts[1].parse().expect("invalid txid in COMMIT");
                    // Apply all operations from this transaction
                    if let Some(ops) = pending.remove(&txid) {
                        for (key, value) in ops {
                            state.insert(key, value);
                        }
                    }
                }
                _ => {
                    panic!("unknown WAL record type: {}", parts[0]);
                }
            }
        }

        // Pending transactions without COMMIT are discarded (already not in state)

        Ok((state, max_txid))
    }

    /// Begin a new transaction.
    pub fn begin(&mut self) -> TxId {
        let txid = self.next_txid;
        self.next_txid += 1;

        writeln!(self.file, "BEGIN {}", txid).expect("failed to write BEGIN");
        crash_point("after_begin_write");
        self.file.sync_all().expect("failed to fsync after BEGIN");
        crash_point("after_wal_fsync");

        txid
    }

    /// Write a key-value pair within a transaction.
    pub fn put(&mut self, txid: TxId, key: &str, value: &str) {
        writeln!(self.file, "PUT {} {} {}", txid, key, value).expect("failed to write PUT");
        crash_point("after_record_write");
    }

    /// Commit a transaction.
    pub fn commit(&mut self, txid: TxId) {
        self.file.sync_all().expect("failed to fsync records");
        writeln!(self.file, "COMMIT {}", txid).expect("failed to write COMMIT");
        crash_point("after_commit_write");
        self.file.sync_all().expect("failed to fsync after COMMIT");
        crash_point("after_wal_fsync");

        let wal_path = self.dir.join("wal.log");
        let (state, _) = Self::recover_from_file(&wal_path).expect("failed to recover after commit");
        self.state = state;
    }


    /// Get a value by key from committed state.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.state.get(key).map(|s| s.as_str())
    }
}
