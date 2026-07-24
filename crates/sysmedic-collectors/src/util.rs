use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Hard limit on how long any single external command may run. A hung tool
/// (e.g. `df` blocking on a dead NFS mount, `smartctl` stalling on a failing
/// USB bridge) must not freeze the whole checkup, which runs collectors
/// sequentially. On timeout the child is killed and the call returns `None`.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// A minimal, known-good `PATH`. External tools are looked up here rather than
/// through the inherited `PATH`, so a poisoned `PATH` cannot substitute an
/// attacker-controlled binary — this matters because the privileged
/// `sysmedic-fix-helper` runs these same collectors as root.
const SAFE_PATH: &str = "/usr/sbin:/usr/bin:/sbin:/bin";

/// The captured result of running a command: whether it exited successfully
/// and its stdout. Unlike [`run`], this is returned even for a non-zero exit,
/// so callers that need the output of tools with bitmask exit codes (notably
/// `smartctl`) can inspect it.
pub struct CommandOutput {
    pub success: bool,
    pub stdout: String,
}

/// Run a command with a fixed locale (`LC_ALL=C`), a sanitized `PATH`, and a
/// timeout, returning its exit-success flag and stdout. Returns `None` only
/// when the binary cannot be spawned or the command times out.
///
/// The locale is pinned so parsers that match on English text (`apt`, `ufw`,
/// `dpkg`) keep working on non-English systems, where those tools are
/// gettext-translated.
pub fn run_captured(cmd: &str, args: &[&str]) -> Option<CommandOutput> {
    let mut child = Command::new(cmd)
        .args(args)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("PATH", SAFE_PATH)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    // Drain stdout on a separate thread so a large output cannot deadlock us
    // against a full pipe buffer while we are waiting on the child.
    let mut pipe = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = pipe.read_to_end(&mut buf);
        buf
    });

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if start.elapsed() >= COMMAND_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    };
    let stdout = reader.join().ok()?;
    Some(CommandOutput {
        success: status.success(),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
    })
}

/// Run a command and return stdout on success, `None` if the binary is
/// missing, fails, exits non-zero, or times out.
pub fn run(cmd: &str, args: &[&str]) -> Option<String> {
    run_captured(cmd, args)
        .filter(|o| o.success)
        .map(|o| o.stdout)
}

pub fn read_file(path: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(path).ok()
}

pub fn read_trimmed(path: impl AsRef<Path>) -> Option<String> {
    read_file(path).map(|s| s.trim().to_string())
}

/// Parse human sizes as printed by journalctl/snap ("3.9G", "512.0M",
/// "16K", "895B", plain bytes).
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let split = s.find(|c: char| !c.is_ascii_digit() && c != '.');
    let (num, unit) = match split {
        Some(i) => s.split_at(i),
        None => (s, ""),
    };
    let value: f64 = num.parse().ok()?;
    let mult: f64 = match unit.trim() {
        "" | "B" => 1.0,
        "K" | "KB" | "KiB" => 1024.0,
        "M" | "MB" | "MiB" => 1024.0 * 1024.0,
        "G" | "GB" | "GiB" => 1024.0 * 1024.0 * 1024.0,
        "T" | "TB" | "TiB" => 1024.0f64.powi(4),
        _ => return None,
    };
    Some((value * mult) as u64)
}

/// Total size of regular files under `dir`, descending at most
/// `max_depth` levels. Unreadable entries are skipped.
pub fn dir_size(dir: impl AsRef<Path>, max_depth: u32) -> u64 {
    fn walk(dir: &Path, depth: u32, total: &mut u64) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_file() {
                *total += meta.len();
            } else if meta.is_dir() && depth > 0 {
                walk(&entry.path(), depth - 1, total);
            }
        }
    }
    let mut total = 0;
    walk(dir.as_ref(), max_depth, &mut total);
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_journalctl_style_sizes() {
        assert_eq!(parse_size("895B"), Some(895));
        assert_eq!(parse_size("16.0K"), Some(16384));
        assert_eq!(parse_size("512.0M"), Some(512 * 1024 * 1024));
        assert_eq!(
            parse_size("3.9G"),
            Some((3.9 * 1024.0 * 1024.0 * 1024.0) as u64)
        );
        assert_eq!(parse_size("1234"), Some(1234));
        assert_eq!(parse_size("bogus"), None);
    }
}
