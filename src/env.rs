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
/// Provided to the verify closure after each crash-restart cycle.
/// Use this to understand which crash point triggered the current
/// verification run.
///
/// # Stability guarantees
///
/// - `point_id` is stable for a given test and crash schedule
/// - `label` is exactly the string passed to `crash_point()`
///
/// # Non-guarantees
///
/// - `point_id` values may change if crash points are added or removed
/// - Labels are not required to be unique
///
/// # Example
///
/// ```ignore
/// .verify(|env, crash| {
///     println!("Verifying after crash point {} ({})", crash.point_id, crash.label);
/// })
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CrashInfo {
    /// The 1-indexed crash point ID.
    ///
    /// This is the order in which the crash point was encountered during
    /// execution. The first crash point hit is `1`, the second is `2`, etc.
    ///
    /// Use this for:
    /// - Logging and debugging
    /// - Identifying which crash point triggered this run
    ///
    /// Reproducing a crash requires running the same test with the same
    /// execution order and crash schedule.
    pub point_id: usize,

    /// The label passed to `crash_point("label")`.
    ///
    /// This is the string provided when the crash point was defined.
    /// Use descriptive labels to make crash reports meaningful.
    ///
    /// # Example labels
    ///
    /// - `"after_wal_write"`
    /// - `"before_manifest_update"`
    /// - `"committed"`
    pub label: String,
}

impl CrashInfo {
    /// Create crash info from parsed metadata.
    pub(crate) fn new(point_id: usize, label: String) -> Self {
        Self { point_id, label }
    }
}
