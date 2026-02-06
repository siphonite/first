//! Test environment and crash information.
//!
//! Provides context to test closures.

use std::path::{Path, PathBuf};

/// Environment provided to test closures.
///
/// Contains the isolated working directory for this test run.
/// This type is opaque; users interact with it only through [`Env::path()`].
pub struct Env {
    work_dir: PathBuf,
}

impl Env {
    /// Create a new environment with the given work directory.
    pub(crate) fn new(work_dir: PathBuf) -> Self {
        Self { work_dir }
    }

    /// Returns an absolute path inside this test's isolated workspace.
    ///
    /// The returned path is guaranteed to:
    /// - Be within a directory created by FIRST
    /// - Have its parent workspace directory exist before `run()` is executed
    /// - Be reset for each crash-restart iteration
    ///
    /// This function performs no I/O beyond path construction.
    /// The caller is responsible for creating any subdirectories or files.
    ///
    /// # Panics
    ///
    /// Panics if `name` is an absolute path. Only relative paths are allowed
    /// to ensure isolation within the workspace.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let db_path = env.path("mydb");
    /// std::fs::create_dir_all(&db_path).unwrap();
    /// ```
    pub fn path(&self, name: impl AsRef<Path>) -> PathBuf {
        let name = name.as_ref();
        assert!(
            !name.is_absolute(),
            "Env::path() requires a relative path, got absolute: {:?}",
            name
        );
        self.work_dir.join(name)
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
