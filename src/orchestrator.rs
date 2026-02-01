//! Orchestrator: the supervisor loop.
//!
//! Manages crash → restart → verify cycles.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

use crate::env::{CrashInfo, Env};

/// Base directory for FIRST test runs.
const FIRST_BASE_DIR: &str = "/tmp/first";

/// Exit code for SIGKILL (128 + 9).
const SIGKILL_EXIT_CODE: i32 = 137;

/// Run the orchestrator loop.
///
/// Iterates through crash points, spawning execution and verification
/// processes for each one.
pub fn run<R, V>(_run_fn: Option<R>, _verify_fn: Option<V>)
where
    R: FnOnce(&Env),
    V: FnOnce(&Env, &CrashInfo),
{
    // Note: We don't actually use the closures here.
    // The orchestrator spawns child processes that re-run the test binary.
    // The closures are stored so the type signature matches, but children
    // will re-parse and call their own closures.

    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[first] error: cannot find current executable: {}", e);
            std::process::exit(1);
        }
    };

    // Try to get test name from args (e.g., `cargo test test_name`)
    let test_name = extract_test_name();

    let mut target: usize = 1;

    loop {
        let work_dir = PathBuf::from(FIRST_BASE_DIR).join(format!("run_{}", target));

        // Create fresh work directory
        if let Err(e) = fs::create_dir_all(&work_dir) {
            eprintln!("[first] error: cannot create {}: {}", work_dir.display(), e);
            std::process::exit(1);
        }

        // Spawn EXECUTION phase
        let exec_result = spawn_child(
            &exe,
            &test_name,
            "EXECUTION",
            target,
            &work_dir,
        );

        match exec_result {
            ChildResult::Crashed(crash_info) => {
                // Child crashed as expected, now verify
                let verify_result = spawn_child_with_crash_info(
                    &exe,
                    &test_name,
                    target,
                    &work_dir,
                    &crash_info,
                );

                match verify_result {
                    ChildResult::Success => {
                        eprintln!("[first] crash point {}: OK", target);
                        // Clean up work dir on success (unless FIRST_KEEP_ARTIFACTS)
                        if std::env::var("FIRST_KEEP_ARTIFACTS").is_err() {
                            let _ = fs::remove_dir_all(&work_dir);
                        }
                    }
                    ChildResult::Failed(code) => {
                        eprintln!(
                            "[first] crash point {}: FAILED (see {})",
                            target,
                            work_dir.display()
                        );
                        eprintln!("[first] verification failed with exit code {}", code);
                        std::process::exit(1);
                    }
                    ChildResult::Crashed(_) => {
                        eprintln!(
                            "[first] crash point {}: FAILED (verify phase crashed unexpectedly)",
                            target
                        );
                        std::process::exit(1);
                    }
                }
            }
            ChildResult::Success => {
                // Child completed normally - no more crash points
                eprintln!("[first] all {} crash points passed", target - 1);
                // Clean up the unused work dir
                let _ = fs::remove_dir_all(&work_dir);
                return;
            }
            ChildResult::Failed(code) => {
                eprintln!(
                    "[first] crash point {}: FAILED (execution failed with exit code {})",
                    target, code
                );
                std::process::exit(1);
            }
        }

        target += 1;
    }
}

/// Result of a child process execution.
enum ChildResult {
    /// Child exited successfully (exit code 0).
    Success,
    /// Child was killed by SIGKILL (crash occurred).
    Crashed(CrashInfo),
    /// Child failed with a non-zero exit code.
    Failed(i32),
}

/// Spawn a child process in the given phase.
fn spawn_child(
    exe: &PathBuf,
    test_name: &Option<String>,
    phase: &str,
    target: usize,
    work_dir: &PathBuf,
) -> ChildResult {
    let mut cmd = Command::new(exe);

    // Set FIRST environment variables
    cmd.env("FIRST_PHASE", phase);
    cmd.env("FIRST_CRASH_TARGET", target.to_string());
    cmd.env("FIRST_WORK_DIR", work_dir.to_string_lossy().to_string());

    // If we know the test name, filter to just that test
    if let Some(name) = test_name {
        cmd.arg(name);
        cmd.arg("--");
        cmd.arg("--exact");
    }

    // Capture stderr to parse crash metadata
    cmd.stderr(Stdio::piped());
    cmd.stdout(Stdio::null());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[first] error: cannot spawn child: {}", e);
            return ChildResult::Failed(1);
        }
    };

    // Read stderr for crash metadata
    let stderr = child.stderr.take();
    let crash_info = stderr.and_then(|s| parse_crash_metadata(s));

    // Wait for child to exit
    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[first] error: cannot wait for child: {}", e);
            return ChildResult::Failed(1);
        }
    };

    interpret_exit_status(status, crash_info)
}

/// Spawn a child process in VERIFY phase with crash info.
fn spawn_child_with_crash_info(
    exe: &PathBuf,
    test_name: &Option<String>,
    target: usize,
    work_dir: &PathBuf,
    crash_info: &CrashInfo,
) -> ChildResult {
    let mut cmd = Command::new(exe);

    // Set FIRST environment variables
    cmd.env("FIRST_PHASE", "VERIFY");
    cmd.env("FIRST_CRASH_TARGET", target.to_string());
    cmd.env("FIRST_WORK_DIR", work_dir.to_string_lossy().to_string());
    cmd.env("FIRST_CRASH_POINT_ID", crash_info.point_id.to_string());
    cmd.env("FIRST_CRASH_LABEL", &crash_info.label);

    // If we know the test name, filter to just that test
    if let Some(name) = test_name {
        cmd.arg(name);
        cmd.arg("--");
        cmd.arg("--exact");
    }

    // Don't capture stderr for verify - let it pass through
    cmd.stderr(Stdio::inherit());
    cmd.stdout(Stdio::null());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[first] error: cannot spawn verify child: {}", e);
            return ChildResult::Failed(1);
        }
    };

    // Wait for child to exit
    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[first] error: cannot wait for verify child: {}", e);
            return ChildResult::Failed(1);
        }
    };

    interpret_exit_status(status, None)
}

/// Parse crash metadata from child's stderr.
fn parse_crash_metadata(stderr: impl std::io::Read) -> Option<CrashInfo> {
    let reader = BufReader::new(stderr);
    for line in reader.lines().map_while(Result::ok) {
        // Look for JSON crash metadata
        if line.starts_with(r#"{"event":"crash""#) {
            // Simple JSON parsing (avoid adding serde dependency for now)
            if let Some(info) = parse_crash_json(&line) {
                return Some(info);
            }
        }
    }
    None
}

/// Simple JSON parser for crash metadata.
fn parse_crash_json(json: &str) -> Option<CrashInfo> {
    // Format: {"event":"crash","point_id":N,"label":"...","seed":...,"work_dir":"..."}
    let point_id = json
        .find(r#""point_id":"#)
        .and_then(|i| {
            let start = i + 11;
            let end = json[start..].find(',')?;
            json[start..start + end].parse().ok()
        })?;

    let label = json
        .find(r#""label":""#)
        .and_then(|i| {
            let start = i + 9;
            let end = json[start..].find('"')?;
            Some(json[start..start + end].to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());

    Some(CrashInfo::new(point_id, label))
}

/// Interpret child exit status.
fn interpret_exit_status(status: ExitStatus, crash_info: Option<CrashInfo>) -> ChildResult {
    if status.success() {
        return ChildResult::Success;
    }

    let code = status.code().unwrap_or(-1);

    if code == SIGKILL_EXIT_CODE {
        // SIGKILL - this is an expected crash
        let info = crash_info.unwrap_or_else(|| CrashInfo::new(0, "unknown".to_string()));
        return ChildResult::Crashed(info);
    }

    // Check for signal on Unix
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            if signal == libc::SIGKILL {
                let info = crash_info.unwrap_or_else(|| CrashInfo::new(0, "unknown".to_string()));
                return ChildResult::Crashed(info);
            }
        }
    }

    ChildResult::Failed(code)
}

/// Extract test name from command line arguments.
fn extract_test_name() -> Option<String> {
    // Look for test name in args
    // Typical: target/debug/deps/first-xxx test_name
    let args: Vec<String> = std::env::args().collect();

    // Skip the executable path, look for something that looks like a test name
    for arg in args.iter().skip(1) {
        // Skip flags
        if arg.starts_with('-') {
            continue;
        }
        // Skip common cargo test args
        if arg == "--" || arg == "--exact" || arg == "--nocapture" {
            continue;
        }
        // This might be the test name
        return Some(arg.clone());
    }

    None
}
