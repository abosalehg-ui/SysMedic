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

/// ufw's own state file. World-readable (0644), so unlike `ufw status` it
/// answers without root.
const UFW_CONF: &str = "/etc/ufw/ufw.conf";

/// Unit-file locations that tell us firewalld is installed at all. Without
/// this check, `systemctl is-active firewalld` on a machine that has never
/// heard of firewalld prints "inactive" — which would be reported as a
/// disabled firewall rather than as no firewall.
const FIREWALLD_UNITS: &[&str] = &[
    "/usr/lib/systemd/system/firewalld.service",
    "/lib/systemd/system/firewalld.service",
    "/etc/systemd/system/firewalld.service",
];

/// Firewall state, in order of authority:
///
/// 1. `ufw status` — the live rule state, but it requires root.
/// 2. `/etc/ufw/ufw.conf` (`ENABLED=yes|no`) — ufw's own persisted setting,
///    readable by anyone.
/// 3. firewalld's unit state, when firewalld is installed.
///
/// Step 2 is why this exists. The only branch that could ever return
/// `Some(false)` was the root-only `ufw status`, and both the
/// `security.firewall_inactive` rule and the `fix.enable_ufw` fix require
/// exactly `Some(false)` — so on the normal unprivileged run (the mode the
/// whole app is designed around) a machine with ufw installed and switched
/// off reported no finding, offered no fix, and scored a clean 100 for
/// Security. An absent answer was being read as a good one.
///
/// `None` still means "no firewall frontend we understand", e.g. a
/// hand-rolled nftables ruleset — the rule stays quiet rather than guessing.
fn firewall_active() -> Option<bool> {
    if let Some(out) = util::run("ufw", &["status"]) {
        return Some(out.contains("Status: active"));
    }
    if let Some(state) = util::read_file(UFW_CONF).and_then(|c| parse_ufw_conf(&c)) {
        return Some(state);
    }
    if FIREWALLD_UNITS
        .iter()
        .any(|p| std::path::Path::new(p).exists())
    {
        // `is-active` exits non-zero for an inactive unit, so capture the
        // output regardless of status rather than treating it as no answer.
        if let Some(out) = util::run_captured("systemctl", &["is-active", "firewalld"]) {
            return Some(out.stdout.trim() == "active");
        }
    }
    None
}

/// `ENABLED=yes|no` out of `/etc/ufw/ufw.conf`, or `None` when the key is
/// absent (which means the file is not ufw's config, so we know nothing).
///
/// The file is `KEY=value` shell-ish syntax with `#` comments; ufw writes
/// `ENABLED=yes`/`ENABLED=no` in place when the firewall is toggled.
pub fn parse_ufw_conf(config: &str) -> Option<bool> {
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("ENABLED") {
            let value = value.trim().trim_matches(['"', '\'']);
            return Some(value.eq_ignore_ascii_case("yes"));
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
    fn ufw_conf_reports_the_disabled_state_without_root() {
        // The case the old detection could never report: ufw installed and
        // switched off, checked by an unprivileged process.
        let conf = "# /etc/ufw/ufw.conf\nENABLED=no\nLOGLEVEL=low\n";
        assert_eq!(parse_ufw_conf(conf), Some(false));
    }

    #[test]
    fn ufw_conf_reports_the_enabled_state() {
        assert_eq!(parse_ufw_conf("ENABLED=yes\nLOGLEVEL=low\n"), Some(true));
        // Quoted and oddly-cased values are still understood.
        assert_eq!(parse_ufw_conf("enabled=\"YES\"\n"), Some(true));
        assert_eq!(parse_ufw_conf("ENABLED = no\n"), Some(false));
    }

    #[test]
    fn ufw_conf_without_the_key_is_unknown() {
        assert_eq!(parse_ufw_conf("LOGLEVEL=low\n"), None);
        assert_eq!(parse_ufw_conf("#ENABLED=yes\n"), None);
        assert_eq!(parse_ufw_conf(""), None);
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
