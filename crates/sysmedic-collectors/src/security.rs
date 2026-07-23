use sysmedic_core::snapshot::SecurityInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct SecurityCollector;

impl Collector for SecurityCollector {
    fn name(&self) -> &'static str {
        "security"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let sshd = util::read_file("/etc/ssh/sshd_config");
        snapshot.security = Some(SecurityInfo {
            firewall_active: firewall_active(),
            ssh_permit_root_login: sshd.as_deref().and_then(parse_permit_root_login),
            ssh_password_auth: sshd.as_deref().and_then(parse_password_auth),
        });
    }
}

/// Effective `PasswordAuthentication` (last directive wins). `None` when the
/// directive is absent — sshd's built-in default is `yes`, but we only flag
/// what is explicitly configured to avoid false positives.
pub fn parse_password_auth(config: &str) -> Option<bool> {
    let mut value = None;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("PasswordAuthentication") {
            value = Some(rest.trim().eq_ignore_ascii_case("yes"));
        }
    }
    value
}

/// Best-effort firewall detection: `ufw status` when runnable (needs
/// root), otherwise whether the ufw/firewalld service is active.
fn firewall_active() -> Option<bool> {
    if let Some(out) = util::run("ufw", &["status"]) {
        return Some(out.contains("Status: active"));
    }
    for service in ["ufw", "firewalld"] {
        if let Some(out) = util::run("systemctl", &["is-active", service]) {
            if out.trim() == "active" {
                return Some(true);
            }
        }
    }
    None
}

/// Effective `PermitRootLogin` from sshd_config: `Some(true)` only for an
/// explicit `yes` (last directive wins). `None` when the directive is
/// absent (upstream default is prohibit-password).
pub fn parse_permit_root_login(config: &str) -> Option<bool> {
    let mut value = None;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("PermitRootLogin") {
            let v = rest.trim().to_ascii_lowercase();
            value = Some(v == "yes");
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_yes_is_flagged() {
        assert_eq!(parse_permit_root_login("PermitRootLogin yes\n"), Some(true));
    }

    #[test]
    fn prohibit_password_is_safe() {
        assert_eq!(
            parse_permit_root_login("PermitRootLogin prohibit-password\n"),
            Some(false)
        );
    }

    #[test]
    fn commented_directive_is_ignored() {
        assert_eq!(parse_permit_root_login("#PermitRootLogin yes\n"), None);
    }

    #[test]
    fn password_auth_parsed() {
        assert_eq!(
            parse_password_auth("PasswordAuthentication yes\n"),
            Some(true)
        );
        assert_eq!(
            parse_password_auth("PasswordAuthentication no\n"),
            Some(false)
        );
        assert_eq!(parse_password_auth("# nothing here\n"), None);
    }
}
