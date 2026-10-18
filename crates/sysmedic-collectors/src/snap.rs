use std::path::Path;

use sysmedic_core::snapshot::SnapInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct SnapCollector;

impl Collector for SnapCollector {
    fn name(&self) -> &'static str {
        "snap"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let Some(list) = util::run("snap", &["list", "--all"]) else {
            return; // snapd not installed
        };
        let snaps_dir = Path::new("/var/lib/snapd/snaps");
        snapshot.snap = Some(SnapInfo {
            disabled_revisions: parse_disabled_revisions(&list),
            excess_revisions: parse_excess_revisions(&list),
            snaps_dir_bytes: snaps_dir.is_dir().then(|| util::dir_size(snaps_dir, 0)),
            // Captured so undoing `fix.snap_retain` can put back the value
            // this system had, rather than snapd's default.
            refresh_retain: util::run("snap", &["get", "system", "refresh.retain"])
                .and_then(|s| parse_refresh_retain(&s)),
        });
    }
}

/// The `refresh.retain` value from `snap get system refresh.retain`.
///
/// snapd prints the bare number, and an unset option makes the command fail
/// (so we never see output) — both cases end up as `None`, meaning "we do not
/// know what to restore".
pub fn parse_refresh_retain(s: &str) -> Option<u32> {
    s.trim().parse().ok()
}

/// Whether a `snap list --all` row is a disabled (old) revision.
fn is_disabled(line: &str) -> bool {
    line.split_whitespace()
        .any(|field| field.split(',').any(|note| note == "disabled"))
}

/// Old (disabled) revisions kept on disk, from `snap list --all`.
pub fn parse_disabled_revisions(s: &str) -> u32 {
    s.lines().skip(1).filter(|line| is_disabled(line)).count() as u32
}

/// The rollback copies snapd keeps per snap at the retention the fix sets
/// (`refresh.retain=2`: the current revision plus this many disabled ones).
const ROLLBACKS_KEPT: u32 = 1;

/// Disabled revisions beyond the one rollback copy per snap, from
/// `snap list --all` — the count that is actually reclaimable by lowering
/// `refresh.retain`.
pub fn parse_excess_revisions(s: &str) -> u32 {
    let mut per_snap: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    for line in s.lines().skip(1).filter(|line| is_disabled(line)) {
        if let Some(name) = line.split_whitespace().next() {
            *per_snap.entry(name).or_default() += 1;
        }
    }
    per_snap
        .values()
        .map(|n| n.saturating_sub(ROLLBACKS_KEPT))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_disabled_revisions() {
        let fixture = "\
Name     Version  Rev    Tracking       Publisher   Notes
core22   20240111 1122   latest/stable  canonical✓  base,disabled
core22   20240220 1380   latest/stable  canonical✓  base
firefox  122.0    3836   latest/stable  mozilla✓    disabled
firefox  123.0    3941   latest/stable  mozilla✓    -
";
        assert_eq!(parse_disabled_revisions(fixture), 2);
        // One disabled revision per snap is the rollback retain=2 keeps:
        // nothing here is excess, so nothing to fix.
        assert_eq!(parse_excess_revisions(fixture), 0);
    }

    #[test]
    fn excess_counts_only_beyond_one_rollback_per_snap() {
        // firefox holds three old revisions (two beyond its rollback), core22
        // holds one (none beyond): 2 in total, not the 4 disabled rows.
        let fixture = "\
Name     Version  Rev    Tracking       Publisher   Notes
core22   20240111 1122   latest/stable  canonical✓  base,disabled
core22   20240220 1380   latest/stable  canonical✓  base
firefox  120.0    3600   latest/stable  mozilla✓    disabled
firefox  121.0    3700   latest/stable  mozilla✓    disabled
firefox  122.0    3836   latest/stable  mozilla✓    disabled
firefox  123.0    3941   latest/stable  mozilla✓    -
";
        assert_eq!(parse_disabled_revisions(fixture), 4);
        assert_eq!(parse_excess_revisions(fixture), 2);
        assert_eq!(parse_excess_revisions(""), 0);
    }

    #[test]
    fn parses_refresh_retain() {
        assert_eq!(parse_refresh_retain("10\n"), Some(10));
        assert_eq!(parse_refresh_retain(" 3 "), Some(3));
        // An unset or unparseable option means we have nothing to restore.
        assert_eq!(parse_refresh_retain(""), None);
        assert_eq!(
            parse_refresh_retain("error: no \"refresh.retain\" set"),
            None
        );
    }
}
