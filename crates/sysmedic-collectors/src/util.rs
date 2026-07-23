use std::fs;
use std::path::Path;
use std::process::Command;

/// Run a command and return stdout on success, `None` if the binary is
/// missing, fails, or exits non-zero.
pub fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(cmd).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
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
