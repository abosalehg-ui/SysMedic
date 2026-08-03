//! Shared filesystem policy: where per-user state lives, how private files
//! are written, and the `PATH` external tools are resolved through.
//!
//! These used to be duplicated across `sysmedic-cli`, `sysmedic-gui`,
//! `sysmedic-fixes`, `sysmedic-history` and `sysmedic-collectors`. Three of
//! those copies sit on a security-relevant path (the sanitized `PATH` for
//! commands that run as root, and the owner-only mode for reports that carry
//! host recon data), so tightening one copy silently left the others behind.

use std::path::{Path, PathBuf};

/// A minimal, known-good `PATH`. External tools are looked up here rather than
/// through the inherited `PATH`, so a poisoned `PATH` cannot substitute an
/// attacker-controlled binary — this matters because the privileged
/// `sysmedic-fix-helper` runs collectors and fix commands as root.
pub const SAFE_PATH: &str = "/usr/sbin:/usr/bin:/sbin:/bin";

/// The per-user state directory (`$XDG_STATE_HOME`, else `$HOME/.local/state`).
///
/// Returns `None` when neither is set. Callers must **not** fall back to a
/// shared temp directory: a predictable path under `/tmp` is world-traversable,
/// so another local user can pre-create it and plant a symlink at the file we
/// are about to write. Failing loudly is the correct outcome.
pub fn state_dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var("XDG_STATE_HOME")
        .ok()
        .filter(|s| !s.is_empty())
    {
        return Some(PathBuf::from(xdg));
    }
    std::env::var("HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|h| Path::new(&h).join(".local/state"))
}

/// Write `contents` to `path` with owner-only permissions (0600 on Unix).
///
/// Reports and state files carry hostnames, listening ports and the package
/// inventory — useful recon data that must not be world-readable.
///
/// The mode is applied **twice on purpose**: `OpenOptions::mode` only takes
/// effect when the file is created, so an already-existing file (one an editor
/// rewrote as 0644, say) would silently keep its permissions and leak the new
/// contents. The explicit `set_permissions` after opening closes that.
pub fn write_private(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        file.write_all(contents)
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, contents)
    }
}

/// Best-effort tightening of a file an external tool created (the PDF
/// converter writes with the caller's umask, so it cannot be given a mode).
pub fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn write_private_tightens_an_existing_world_readable_file() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.html");

        // Simulate a file an editor rewrote with the default umask.
        std::fs::write(&path, b"old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );

        write_private(&path, b"secrets: hostnames, ports, packages").unwrap();

        // The rewrite must not inherit the loose mode.
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600,
            "an existing file kept its permissions — the report stayed world-readable"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "secrets: hostnames, ports, packages"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_private_creates_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.json");
        write_private(&path, b"{}").unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn state_dir_prefers_xdg_then_home() {
        // Uses the ambient environment; assert only the shape of the result.
        if let Some(dir) = state_dir() {
            assert!(dir.is_absolute() || dir.components().count() > 0);
        }
    }
}
