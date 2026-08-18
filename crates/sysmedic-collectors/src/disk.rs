use sysmedic_core::snapshot::DiskInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct DiskCollector;

/// Filesystem types a "disk nearly full" finding must never fire on.
///
/// Two groups, both of which are 100% "full" by construction:
///
/// * Pseudo filesystems (`tmpfs`, `devtmpfs`, `overlay`, `efivarfs`, `ramfs`)
///   — not disks at all.
/// * Read-only images (`squashfs`, `iso9660`, `udf`, `erofs`) — a mounted ISO,
///   a game image, a distro installer, an AppImage. Every one of them reports
///   zero bytes free, which used to raise a **Critical** `storage.disk_nearly_full`
///   finding, and a Critical caps the whole health score at 59. Plugging in an
///   install USB dropped the machine from "Excellent" to "Poor".
///
/// Applied twice on purpose: as `df -x` arguments (so df never reports them)
/// and again in [`parse_df`], which is the pure, unit-tested half.
const EXCLUDED_FS: &[&str] = &[
    "tmpfs", "devtmpfs", "ramfs", "overlay", "efivarfs", "squashfs", "iso9660", "udf", "erofs",
];

impl Collector for DiskCollector {
    fn name(&self) -> &'static str {
        "disk"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let mut args = vec!["-B1", "--output=source,target,fstype,size,avail"];
        for fs in EXCLUDED_FS {
            args.push("-x");
            args.push(fs);
        }
        let out = util::run("df", &args);
        match out.map(|s| parse_df(&s)) {
            Some(disks) if !disks.is_empty() => snapshot.disks = Some(disks),
            _ => snapshot
                .collection_errors
                .push("disk: df unavailable or returned no filesystems".into()),
        }
    }
}

pub fn parse_df(s: &str) -> Vec<DiskInfo> {
    // Columns are: source target fstype size avail. The target (mount point)
    // may contain spaces, so the three trailing columns are parsed from the
    // right and everything between source and them is the mount point. Entries
    // are de-duplicated by source device, so a filesystem mounted more than
    // once (e.g. a bind mount) is not reported — and diagnosed — twice.
    let mut seen = std::collections::HashSet::new();
    s.lines()
        .skip(1)
        .filter_map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() < 5 {
                return None;
            }
            let n = tokens.len();
            let source = tokens[0];
            let available_bytes: u64 = tokens[n - 1].parse().ok()?;
            let total_bytes: u64 = tokens[n - 2].parse().ok()?;
            let fs_type = tokens[n - 3].to_string();
            let mount_point = tokens[1..n - 3].join(" ");
            if !mount_point.starts_with('/') || total_bytes == 0 {
                return None;
            }
            // Belt and braces: even if the `-x` arguments were not applied
            // (an unusual df, a caller passing raw output), a read-only image
            // must not become a Critical "disk full" finding.
            if EXCLUDED_FS.contains(&fs_type.as_str()) {
                return None;
            }
            if !seen.insert(source.to_string()) {
                return None;
            }
            Some(DiskInfo {
                mount_point,
                fs_type,
                total_bytes,
                available_bytes,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_df_output() {
        let fixture = "\
Filesystem     Mounted on     Type      1B-blocks        Avail
/dev/sda2      /              ext4   502392610816  50239261081
/dev/sda1      /boot/efi      vfat      535805952    529530880
";
        let disks = parse_df(fixture);
        assert_eq!(disks.len(), 2);
        assert_eq!(disks[0].mount_point, "/");
        assert!((disks[0].used_percent() - 90.0).abs() < 0.5);
    }

    #[test]
    fn handles_mount_points_with_spaces() {
        let fixture = "\
Filesystem     Mounted on        Type   1B-blocks       Avail
/dev/sdb1      /media/My Disk    ext4   1000000000   500000000
";
        let disks = parse_df(fixture);
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].mount_point, "/media/My Disk");
        assert_eq!(disks[0].fs_type, "ext4");
        assert_eq!(disks[0].total_bytes, 1_000_000_000);
    }

    #[test]
    fn read_only_images_are_not_reported_as_full_disks() {
        // A mounted ISO and a squashfs image are always 100% full. Reporting
        // them raised a Critical finding that capped the health score at 59.
        let fixture = "\
Filesystem     Mounted on          Type      1B-blocks   Avail
/dev/sda2      /                   ext4     1000000000   500000000
/dev/sr0       /media/user/ubuntu  iso9660  4700000000           0
/dev/loop3     /snap/core22/1122   squashfs   77000000           0
/dev/loop9     /media/img          erofs      12000000           0
";
        let disks = parse_df(fixture);
        assert_eq!(disks.len(), 1, "only the real filesystem should remain");
        assert_eq!(disks[0].mount_point, "/");
    }

    #[test]
    fn deduplicates_bind_mounts_by_source() {
        let fixture = "\
Filesystem     Mounted on     Type   1B-blocks       Avail
/dev/sda2      /              ext4   1000000000   500000000
/dev/sda2      /var/lib/x     ext4   1000000000   500000000
";
        let disks = parse_df(fixture);
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].mount_point, "/");
    }
}
