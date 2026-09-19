//! Executing [`FixCommand`]s, behind a trait so tests never touch the system.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sysmedic_core::fix::FixCommand;

/// How long a single fix command may run before it is killed.
///
/// Fix commands had no limit at all, while every collector command has had a
/// ten-second one for as long as there have been collectors. The asymmetry was
/// backwards: these run **as root**, from a GUI whose only feedback is a
/// "Applying the fix…" toast that never times out. `apt-get` waiting on the
/// dpkg lock another process holds, or on a debconf prompt nobody can see,
/// hung the privileged helper indefinitely with the spinner still turning.
///
/// Generous, because the work is real: `autoremove --purge` on a slow disk,
/// or a journal vacuum over gigabytes, legitimately takes minutes.
const FIX_TIMEOUT: Duration = Duration::from_secs(600);

/// Runs commands. `RealRunner` shells out; tests use `RecordingRunner`.
pub trait CommandRunner: Send + Sync {
    fn run(&self, command: &FixCommand) -> Result<String, String>;
}

/// Executes commands for real (no shell — program + args directly).
pub struct RealRunner;

use sysmedic_core::paths::SAFE_PATH;

impl CommandRunner for RealRunner {
    fn run(&self, command: &FixCommand) -> Result<String, String> {
        let mut child = Command::new(&command.program)
            .args(&command.args)
            .env("PATH", SAFE_PATH)
            // A fix runs unattended behind a polkit prompt: there is no
            // terminal to answer a question on. `DEBIAN_FRONTEND` tells
            // apt/dpkg not to ask one, and a closed stdin means a tool that
            // asks anyway reads EOF and gives up instead of blocking forever
            // on a prompt no human will ever see.
            .env("DEBIAN_FRONTEND", "noninteractive")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to launch `{}`: {e}", command.program))?;

        // Drain both pipes on their own threads: a command that fills a pipe
        // buffer while we wait on the child would deadlock against us.
        let mut out_pipe = child.stdout.take();
        let mut err_pipe = child.stderr.take();
        let out_reader = std::thread::spawn(move || read_all(out_pipe.as_mut()));
        let err_reader = std::thread::spawn(move || read_all(err_pipe.as_mut()));

        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if start.elapsed() >= FIX_TIMEOUT {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(format!(
                            "`{}` was still running after {} seconds and was stopped; \
                             the system may have been left partly changed",
                            command.display(),
                            FIX_TIMEOUT.as_secs()
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(format!(
                        "`{}` could not be waited on: {e}",
                        command.display()
                    ))
                }
            }
        };

        let stdout = out_reader.join().unwrap_or_default();
        let stderr = err_reader.join().unwrap_or_default();
        if status.success() {
            Ok(stdout.trim().to_string())
        } else {
            Err(format!(
                "`{}` exited with {}: {}",
                command.display(),
                status,
                stderr.trim()
            ))
        }
    }
}

fn read_all(pipe: Option<&mut impl Read>) -> String {
    let Some(pipe) = pipe else {
        return String::new();
    };
    let mut buf = Vec::new();
    let _ = pipe.read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
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
