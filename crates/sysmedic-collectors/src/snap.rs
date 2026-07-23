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
            snaps_dir_bytes: snaps_dir.is_dir().then(|| util::dir_size(snaps_dir, 0)),
        });
    }
}

/// Old (disabled) revisions kept on disk, from `snap list --all`.
pub fn parse_disabled_revisions(s: &str) -> u32 {
    s.lines()
        .skip(1)
        .filter(|line| {
            line.split_whitespace()
                .any(|field| field.split(',').any(|note| note == "disabled"))
        })
        .count() as u32
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
    }
}
