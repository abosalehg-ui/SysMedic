use sysmedic_core::snapshot::SecurityInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct SecurityCollector;

impl Collector for SecurityCollector {
    fn name(&self) -> &'static str {
        "security"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let sshd = read_effective_sshd_config();
        snapshot.security = Some(SecurityInfo {
            firewall_active: firewall_active(),
            ssh_permit_root_login: sshd
                .as_deref()
                .and_then(|c| parse_directive_bool(c, "PermitRootLogin")),
            ssh_password_auth: sshd
                .as_deref()
                .and_then(|c| parse_directive_bool(c, "PasswordAuthentication")),
        });
    }
}

/// Read `/etc/ssh/sshd_config` and splice in any `Include`d drop-in files at the
/// point they appear, so the *effective* configuration is parsed. On Debian ≥
/// 12 / Ubuntu ≥ 22.04 the shipped `sshd_config` puts `Include
/// /etc/ssh/sshd_config.d/*.conf` at the top, so those files hold the real
/// settings (e.g. cloud-init's `PasswordAuthentication`).
fn read_effective_sshd_config() -> Option<String> {
    let main = util::read_file("/etc/ssh/sshd_config")?;
    Some(expand_includes(&main, 0))
}

fn expand_includes(config: &str, depth: u8) -> String {
    let mut out = String::new();
    for line in config.lines() {
        let mut toks = line.split_whitespace();
        let first = toks.next().unwrap_or("");
        if depth < 4 && first.eq_ignore_ascii_case("include") {
            if let Some(glob) = toks.next() {
                for path in glob_conf_files(glob) {
                    if let Some(inc) = util::read_file(&path) {
                        out.push_str(&expand_includes(&inc, depth + 1));
                        out.push('\n');
                    }
                }
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Resolve an sshd `Include` argument supporting a single `*` wildcard in the
/// filename (the shape ssh actually ships), returning matching paths sorted.
fn glob_conf_files(glob: &str) -> Vec<String> {
    let path = std::path::Path::new(glob);
    let (Some(dir), Some(file)) = (path.parent(), path.file_name().and_then(|f| f.to_str())) else {
        return Vec::new();
    };
    let Some(star) = file.find('*') else {
        return if path.exists() {
            vec![glob.to_string()]
        } else {
            Vec::new()
        };
    };
    let (prefix, suffix) = (&file[..star], &file[star + 1..]);
    let mut matches: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (name.starts_with(prefix) && name.ends_with(suffix))
                .then(|| e.path().to_string_lossy().into_owned())
        })
        .collect();
    matches.sort();
    matches
}

/// The effective boolean value of an sshd_config keyword.
///
/// sshd uses the **first** obtained value for each keyword (not the last), so
/// we return the first uncommented occurrence. Parsing stops at the first
/// `Match` block, since directives there are conditional and must not be read
/// as the global setting. Keyword matching is case-insensitive with a word
/// boundary. `Some(true)` only for an explicit `yes`; `None` when unset (we
/// flag only what is explicitly configured, to avoid false positives).
pub fn parse_directive_bool(config: &str, keyword: &str) -> Option<bool> {
    for line in config.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut toks = line.split_whitespace();
        let key = toks.next().unwrap_or("");
        if key.eq_ignore_ascii_case("match") {
            break;
        }
        if key.eq_ignore_ascii_case(keyword) {
            return Some(toks.next().unwrap_or("").eq_ignore_ascii_case("yes"));
        }
    }
    None
}

/// Effective `PasswordAuthentication`. See [`parse_directive_bool`].
pub fn parse_password_auth(config: &str) -> Option<bool> {
    parse_directive_bool(config, "PasswordAuthentication")
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

/// Effective `PermitRootLogin` from sshd_config. See [`parse_directive_bool`].
pub fn parse_permit_root_login(config: &str) -> Option<bool> {
    parse_directive_bool(config, "PermitRootLogin")
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

    #[test]
    fn first_match_wins_not_last() {
        // sshd uses the first obtained value; a stray later `yes` must not win.
        let cfg = "PermitRootLogin no\nPermitRootLogin yes\n";
        assert_eq!(parse_permit_root_login(cfg), Some(false));
    }

    #[test]
    fn keyword_is_case_insensitive() {
        assert_eq!(parse_permit_root_login("permitrootlogin yes\n"), Some(true));
    }

    #[test]
    fn directives_inside_match_blocks_are_ignored() {
        let cfg = "PermitRootLogin no\nMatch Address 10.0.0.0/8\n    PermitRootLogin yes\n";
        assert_eq!(parse_permit_root_login(cfg), Some(false));
    }

    #[test]
    fn word_boundary_avoids_prefix_false_match() {
        // A hypothetical longer keyword sharing the prefix must not match.
        assert_eq!(
            parse_directive_bool("PermitRootLoginXYZ yes\n", "PermitRootLogin"),
            None
        );
    }
}
