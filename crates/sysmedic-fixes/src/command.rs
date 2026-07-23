//! Executing [`FixCommand`]s, behind a trait so tests never touch the system.

use std::process::Command;

use sysmedic_core::fix::FixCommand;

/// Runs commands. `RealRunner` shells out; tests use `RecordingRunner`.
pub trait CommandRunner: Send + Sync {
    fn run(&self, command: &FixCommand) -> Result<String, String>;
}

/// Executes commands for real (no shell — program + args directly).
pub struct RealRunner;

impl CommandRunner for RealRunner {
    fn run(&self, command: &FixCommand) -> Result<String, String> {
        let output = Command::new(&command.program)
            .args(&command.args)
            .output()
            .map_err(|e| format!("failed to launch `{}`: {e}", command.program))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() {
            Ok(stdout.trim().to_string())
        } else {
            Err(format!(
                "`{}` exited with {}: {}",
                command.display(),
                output.status,
                stderr.trim()
            ))
        }
    }
}

/// Records the commands it is asked to run and returns canned output,
/// without executing anything. For unit tests.
#[derive(Default)]
pub struct RecordingRunner {
    pub executed: std::sync::Mutex<Vec<FixCommand>>,
    pub fail_on: Option<String>,
}

impl RecordingRunner {
    pub fn new() -> Self {
        RecordingRunner::default()
    }

    /// Make the runner fail when it sees a command whose program equals
    /// `program` (used to test error handling).
    pub fn failing_on(program: &str) -> Self {
        RecordingRunner {
            executed: std::sync::Mutex::new(Vec::new()),
            fail_on: Some(program.to_string()),
        }
    }

    pub fn commands(&self) -> Vec<FixCommand> {
        self.executed.lock().unwrap().clone()
    }
}

impl CommandRunner for RecordingRunner {
    fn run(&self, command: &FixCommand) -> Result<String, String> {
        if self.fail_on.as_deref() == Some(command.program.as_str()) {
            return Err(format!("simulated failure of {}", command.program));
        }
        self.executed.lock().unwrap().push(command.clone());
        Ok(String::new())
    }
}
