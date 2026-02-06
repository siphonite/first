//! Test builder and phase routing.
//!
//! Provides the `first::test()` API.

use std::path::PathBuf;

use crate::env::{CrashInfo, Env};
use crate::rt::{Phase, runtime};

/// Builder for FIRST tests.
///
/// This type is an implementation detail. Users should interact with it
/// through the builder methods returned by [`test()`].
#[doc(hidden)]
pub struct TestBuilder<R, V>
where
    R: FnOnce(&Env),
    V: FnOnce(&Env, &CrashInfo),
{
    run_fn: Option<R>,
    verify_fn: Option<V>,
}

/// Start building a FIRST test.
///
/// # Example
///
/// ```ignore
/// first::test()
///     .run(|env| {
///         // Workload
///     })
///     .verify(|env, crash_info| {
///         // Recovery + invariants
///     })
///     .execute();
/// ```
#[allow(clippy::type_complexity)]
pub fn test() -> TestBuilder<fn(&Env), fn(&Env, &CrashInfo)> {
    TestBuilder {
        run_fn: None,
        verify_fn: None,
    }
}

impl<R, V> TestBuilder<R, V>
where
    R: FnOnce(&Env),
    V: FnOnce(&Env, &CrashInfo),
{
    /// Define the workload to execute.
    ///
    /// This closure runs during the EXECUTION phase.
    ///
    /// # Panics
    ///
    /// If the closure panics, the EXECUTION phase fails and the orchestrator
    /// reports this as an error. Panics are not treated as crashes.
    ///
    /// # Completion Without Crash
    ///
    /// If the closure completes normally without hitting the target crash point,
    /// the orchestrator interprets this as "schedule exhausted" — all crash
    /// points have been explored and the test passes.
    pub fn run<R2>(self, f: R2) -> TestBuilder<R2, V>
    where
        R2: FnOnce(&Env),
    {
        TestBuilder {
            run_fn: Some(f),
            verify_fn: self.verify_fn,
        }
    }

    /// Define the verification logic.
    ///
    /// This closure runs during the VERIFY phase after a crash.
    ///
    /// # Panics
    ///
    /// If the closure panics (e.g., `assert!` or `panic!`), the VERIFY phase
    /// fails and the test reports an invariant violation. This is the expected
    /// way to signal that crash recovery failed.
    pub fn verify<V2>(self, f: V2) -> TestBuilder<R, V2>
    where
        V2: FnOnce(&Env, &CrashInfo),
    {
        TestBuilder {
            run_fn: self.run_fn,
            verify_fn: Some(f),
        }
    }

    /// Execute the test based on current phase.
    ///
    /// - Orchestrator: runs the supervisor loop
    /// - Execution: calls run closure
    /// - Verify: calls verify closure
    ///
    /// # Error Behavior
    ///
    /// | Phase       | Panic         | Normal Exit        | SIGKILL           |
    /// |-------------|---------------|--------------------|-----------------  |
    /// | Execution   | Test fails    | Schedule exhausted | Crash (expected)  |
    /// | Verify      | Test fails    | Verification OK    | Test fails        |
    pub fn execute(self) {
        let config = runtime();
        let work_dir = std::env::var("FIRST_WORK_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("first").join("default"));

        match config.phase {
            Phase::Orchestrator => {
                // Step 3 will implement this
                crate::orchestrator::run(self.run_fn, self.verify_fn);
            }
            Phase::Execution => {
                if let Some(run_fn) = self.run_fn {
                    let env = Env::new(work_dir);
                    run_fn(&env);
                }
            }
            Phase::Verify => {
                if let Some(verify_fn) = self.verify_fn {
                    let env = Env::new(work_dir);
                    // Parse crash info from env var
                    let crash_info = parse_crash_info();
                    verify_fn(&env, &crash_info);
                }
            }
        }
    }
}

/// Parse crash info from environment variable.
fn parse_crash_info() -> CrashInfo {
    let point_id = std::env::var("FIRST_CRASH_POINT_ID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let label = std::env::var("FIRST_CRASH_LABEL").unwrap_or_else(|_| "unknown".to_string());
    CrashInfo::new(point_id, label)
}
