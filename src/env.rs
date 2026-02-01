//! Test environment and crash information.
//!
//! Provides context to test closures.

use std::path::PathBuf;

/// Environment provided to test closures.
///
/// Contains the isolated working directory for this test run.
pub struct Env {
    work_dir: PathBuf,
}

impl Env {
    /// Create a new environment with the given work directory.
    pub(crate) fn new(work_dir: PathBuf) -> Self {
        Self { work_dir }
    }

    /// Returns a path within the isolated working directory.
    ///
    /// # Arguments
    ///
    /// * `name` - Relative path within the work directory.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let file_path = env.path("data/file.txt");
    /// std::fs::write(&file_path, "hello").unwrap();
    /// ```
    pub fn path(&self, name: &str) -> PathBuf {
        self.work_dir.join(name)
    }

    /// Returns the root of the isolated working directory.
    pub fn root(&self) -> &PathBuf {
        &self.work_dir
    }
}

/// Information about a crash that occurred.
///
/// Provided to the verify closure after a crash.
#[derive(Debug, Clone)]
pub struct CrashInfo {
    /// The crash point ID (1-indexed).
    pub point_id: usize,
    /// The label passed to `crash_point()`.
    pub label: String,
}

impl CrashInfo {
    /// Create crash info from parsed metadata.
    pub(crate) fn new(point_id: usize, label: String) -> Self {
        Self { point_id, label }
    }
}
