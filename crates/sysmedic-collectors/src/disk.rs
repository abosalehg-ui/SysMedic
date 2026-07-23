use sysmedic_core::snapshot::DiskInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct DiskCollector;

impl Collector for DiskCollector {
    fn name(&self) -> &'static str {
        "disk"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        let out = util::run(
            "df",
            &[
                "-B1",
                "--output=target,fstype,size,avail",
                "-x",
                "tmpfs",
                "-x",
                "devtmpfs",
                "-x",
                "squashfs",
                "-x",
                "overlay",
                "-x",
                "efivarfs",
            ],
        );
        match out.map(|s| parse_df(&s)) {
            Some(disks) if !disks.is_empty() => snapshot.disks = Some(disks),
            _ => snapshot
                .collection_errors
                .push("disk: df unavailable or returned no filesystems".into()),
        }
    }
}

pub fn parse_df(s: &str) -> Vec<DiskInfo> {
    s.lines()
        .skip(1)
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let mount_point = it.next()?.to_string();
            let fs_type = it.next()?.to_string();
            let total_bytes: u64 = it.next()?.parse().ok()?;
            let available_bytes: u64 = it.next()?.parse().ok()?;
            if !mount_point.starts_with('/') || total_bytes == 0 {
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
Mounted on     Type      1B-blocks        Avail
/              ext4   502392610816  50239261081
/boot/efi      vfat      535805952    529530880
";
        let disks = parse_df(fixture);
        assert_eq!(disks.len(), 2);
        assert_eq!(disks[0].mount_point, "/");
        assert!((disks[0].used_percent() - 90.0).abs() < 0.5);
    }
}
