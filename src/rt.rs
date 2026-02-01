//! FIRST Runtime: Crash point management and process termination.
//!
//! This module contains the core primitives for crash injection.

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// Global counter tracking the number of crash points encountered.
/// Starts at 0, incremented to 1 on first crash_point, etc.
static CRASH_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Cached runtime configuration, initialized once from environment variables.
static RUNTIME: OnceLock<RuntimeConfig> = OnceLock::new();

/// Environment variable names used by FIRST.
const ENV_PHASE: &str = "FIRST_PHASE";
const ENV_CRASH_TARGET: &str = "FIRST_CRASH_TARGET";
const ENV_WORK_DIR: &str = "FIRST_WORK_DIR";
const ENV_SEED: &str = "FIRST_SEED";

/// Execution phase of the current process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    /// Orchestrator: manages test lifecycle, does not run workload directly.
    Orchestrator,
    /// Execution: runs workload, may crash at target point.
    Execution,
    /// Verify: runs recovery and invariant checks.
    Verify,
}

/// Cached runtime configuration.
#[derive(Debug)]
pub(crate) struct RuntimeConfig {
    pub(crate) phase: Phase,
    /// Target crash point (1-indexed per design doc).
    /// `usize::MAX` means "never crash".
    target_crash_point: usize,
}

/// Initialize the runtime from environment variables.
/// Called once and cached via OnceLock.
fn init_runtime() -> RuntimeConfig {
    let phase = match std::env::var(ENV_PHASE).as_deref() {
        Ok("EXECUTION") => Phase::Execution,
        Ok("VERIFY") => Phase::Verify,
        _ => Phase::Orchestrator,
    };

    let target_crash_point = if phase == Phase::Execution {
        std::env::var(ENV_CRASH_TARGET)
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(usize::MAX)
    } else {
        usize::MAX
    };

    RuntimeConfig {
        phase,
        target_crash_point,
    }
}

/// Returns the cached runtime configuration.
/// Environment variables are read exactly once per process.
#[inline]
pub(crate) fn runtime() -> &'static RuntimeConfig {
    RUNTIME.get_or_init(init_runtime)
}

/// A potential crash location in the execution.
///
/// When FIRST is active during the "Execution" phase, this function:
/// 1. Increments the global crash counter.
/// 2. If `counter == target`, terminates the process immediately via SIGKILL.
/// 3. Otherwise, returns normally.
///
/// When FIRST is inactive (Orchestrator or Verify phase), this is a no-op.
///
/// # Arguments
///
/// * `label` - A descriptive name for this crash point (required).
///   The label is used only for logging/debugging; it does not affect crash
///   scheduling. The label must be a static string or outlive the call.
///   Dynamic allocations are discouraged for performance.
///
/// # Crash Point Numbering
///
/// Crash points are **1-indexed**:
/// - First `crash_point()` call → ID 1
/// - Second `crash_point()` call → ID 2
/// - etc.
///
/// This matches the design spec where `target=1` crashes at the first point.
///
/// # Example
///
/// ```
/// use first::crash_point;
///
/// // These are no-ops when not in EXECUTION phase
/// crash_point("after_write");   // Would be ID 1 in EXECUTION phase
/// crash_point("after_sync");    // Would be ID 2 in EXECUTION phase
/// ```
pub fn crash_point(label: &str) {
    let config = runtime();

    if config.phase != Phase::Execution {
        // No-op in Orchestrator or Verify phases.
        // Fast path: no atomic operations, no allocations.
        return;
    }

    // Increment counter FIRST, then check.
    // fetch_add returns the OLD value, so we add 1 to get the new (1-indexed) ID.
    // This is the most sensitive line in the framework - do not change without
    // updating the design doc (002_crash_point.md).
    let previous = CRASH_COUNTER.fetch_add(1, Ordering::SeqCst);
    let current_id = previous + 1; // 1-indexed: first call = 1, second = 2, etc.

    // SeqCst is used to guarantee deterministic ordering even if users
    // accidentally introduce concurrency in v0.1. This is intentionally
    // conservative; do not "optimize" to weaker orderings.
    let target = config.target_crash_point;

    if current_id == target {
        emit_crash_metadata(current_id, label);
        trigger_crash();
    }
}

/// Emit crash metadata to stderr before killing the process.
/// This allows the Orchestrator to parse what happened.
fn emit_crash_metadata(point_id: usize, label: &str) {
    let seed = std::env::var(ENV_SEED).unwrap_or_else(|_| "null".to_string());
    let work_dir = std::env::var(ENV_WORK_DIR).unwrap_or_else(|_| "unknown".to_string());

    // Write JSON to stderr (flush immediately to avoid loss on SIGKILL)
    let metadata = format!(
        r#"{{"event":"crash","point_id":{},"label":"{}","seed":{},"work_dir":"{}"}}"#,
        point_id,
        label.replace('\\', "\\\\").replace('"', "\\\""),
        seed,
        work_dir.replace('\\', "\\\\").replace('"', "\\\"")
    );

    // Use raw write to stderr to minimize buffering
    let _ = std::io::stderr().write_all(metadata.as_bytes());
    let _ = std::io::stderr().write_all(b"\n");
    let _ = std::io::stderr().flush();
}

/// Terminate the process immediately using SIGKILL.
///
/// This simulates power loss:
/// - No destructors run
/// - No cleanup handlers execute
/// - No buffered I/O is flushed
/// - Filesystem state is left exactly as-is
fn trigger_crash() -> ! {
    // SIGKILL cannot be caught, blocked, or ignored.
    // This is the closest simulation of power loss.
    unsafe {
        libc::kill(libc::getpid(), libc::SIGKILL);
    }

    // Unreachable, but required for `-> !` return type.
    // If SIGKILL somehow fails, fall back to process exit.
    std::process::exit(137) // 128 + 9 (SIGKILL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crash_point_noop_in_orchestrator() {
        // When not in EXECUTION phase, crash_point should be a no-op
        // This test runs in the default (Orchestrator) phase
        crash_point("test_point_1");
        crash_point("test_point_2");
        // If we reach here, crash_point correctly did nothing
        assert!(true);
    }

    #[test]
    fn test_counter_stays_zero_in_orchestrator() {
        // Reset counter for this test
        CRASH_COUNTER.store(0, Ordering::SeqCst);

        // In non-Execution phase, counter should NOT increment
        // (crash_point returns early before incrementing)
        let before = CRASH_COUNTER.load(Ordering::SeqCst);
        crash_point("test");
        let after = CRASH_COUNTER.load(Ordering::SeqCst);

        // Counter stays the same because we're not in Execution phase
        assert_eq!(before, after);
    }

    #[test]
    fn test_runtime_is_cached() {
        // Call runtime() multiple times to verify caching
        let r1 = runtime();
        let r2 = runtime();
        // Should be the same reference (cached)
        assert!(std::ptr::eq(r1, r2));
    }
}
