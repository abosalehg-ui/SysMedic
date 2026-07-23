//! The concrete fixes. Each turns a [`Snapshot`] into a [`FixPlan`] (or
//! `None` when not applicable). Plans are pure data — building one runs no
//! commands — so every fix is unit-tested against fixture snapshots.

use sysmedic_core::fix::{FixCommand, FixPlan};
use sysmedic_core::{Severity, Snapshot};

/// One safe fix SysMedic can offer.
pub trait Fix: Send + Sync {
    /// Stable id, e.g. `fix.apt_clean`.
    fn id(&self) -> &'static str;
    /// Build the plan for this system, or `None` if there is nothing to do.
    fn plan(&self, snapshot: &Snapshot) -> Option<FixPlan>;
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

pub struct AptClean;
impl Fix for AptClean {
    fn id(&self) -> &'static str {
        "fix.apt_clean"
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        let bytes = s.packages.as_ref()?.apt_cache_bytes?;
        if bytes < 100 * 1024 * 1024 {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: "Clear the APT download cache".into(),
            description: format!(
                "Delete cached .deb files in /var/cache/apt/archives, freeing about {:.1} GiB. \
                 Packages re-download on demand if needed.",
                gib(bytes)
            ),
            commands: vec![FixCommand::new("apt-get", &["clean"])],
            affected_paths: vec!["/var/cache/apt/archives".into()],
            reversible: false,
            undo: vec![],
            risk: Severity::Low,
            needs_root: true,
        })
    }
}

pub struct JournalVacuum;
impl Fix for JournalVacuum {
    fn id(&self) -> &'static str {
        "fix.journal_vacuum"
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        let bytes = s.logs.as_ref()?.journal_bytes?;
        if bytes < 1024 * 1024 * 1024 {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: "Trim the systemd journal".into(),
            description: format!(
                "The journal currently uses {:.1} GiB. This keeps the most recent 200 MB of \
                 logs and deletes older archived entries.",
                gib(bytes)
            ),
            commands: vec![FixCommand::new("journalctl", &["--vacuum-size=200M"])],
            affected_paths: vec!["/var/log/journal".into()],
            reversible: false,
            undo: vec![],
            risk: Severity::Low,
            needs_root: true,
        })
    }
}

pub struct AutoremoveKernels;
impl Fix for AutoremoveKernels {
    fn id(&self) -> &'static str {
        "fix.autoremove"
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        let kernels = &s.packages.as_ref()?.old_kernels;
        if kernels.len() <= 2 {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: "Remove old kernels and orphaned packages".into(),
            description: format!(
                "Purge {} old kernel image(s) and any auto-installed packages no longer needed. \
                 The running kernel and one fallback are always kept.",
                kernels.len()
            ),
            commands: vec![FixCommand::new("apt-get", &["autoremove", "--purge", "-y"])],
            affected_paths: vec!["/boot".into(), "/lib/modules".into()],
            reversible: false,
            undo: vec![],
            risk: Severity::Medium,
            needs_root: true,
        })
    }
}

pub struct SnapRetain;
impl Fix for SnapRetain {
    fn id(&self) -> &'static str {
        "fix.snap_retain"
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        let snap = s.snap.as_ref()?;
        if snap.disabled_revisions == 0 {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: "Keep fewer old snap revisions".into(),
            description: format!(
                "Set snapd to retain only 2 revisions per snap (currently {} disabled \
                 revision(s) are held). snapd prunes the extras on the next refresh.",
                snap.disabled_revisions
            ),
            commands: vec![FixCommand::new(
                "snap",
                &["set", "system", "refresh.retain=2"],
            )],
            affected_paths: vec!["/var/lib/snapd/snaps".into()],
            reversible: true,
            undo: vec![FixCommand::new(
                "snap",
                &["set", "system", "refresh.retain=3"],
            )],
            risk: Severity::Low,
            needs_root: true,
        })
    }
}

pub struct FlatpakRemoveUnused;
impl Fix for FlatpakRemoveUnused {
    fn id(&self) -> &'static str {
        "fix.flatpak_unused"
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        let refs = &s.flatpak.as_ref()?.unused_refs;
        if refs.is_empty() {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: "Remove unused Flatpak runtimes".into(),
            description: format!(
                "Uninstall {} Flatpak runtime(s) that no installed app needs.",
                refs.len()
            ),
            commands: vec![FixCommand::new("flatpak", &["uninstall", "--unused", "-y"])],
            affected_paths: vec!["/var/lib/flatpak".into()],
            reversible: false,
            undo: vec![],
            risk: Severity::Low,
            needs_root: true,
        })
    }
}

pub struct EnableUfw;
impl Fix for EnableUfw {
    fn id(&self) -> &'static str {
        "fix.enable_ufw"
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        if s.security.as_ref()?.firewall_active != Some(false) {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: "Enable the firewall".into(),
            description: "Turn on ufw with its default policy: deny incoming, allow outgoing. \
                 Suitable for a desktop with no server software."
                .into(),
            commands: vec![FixCommand::new("ufw", &["--force", "enable"])],
            affected_paths: vec!["/etc/ufw".into(), "/lib/systemd/system/ufw.service".into()],
            reversible: true,
            undo: vec![FixCommand::new("ufw", &["disable"])],
            risk: Severity::Low,
            needs_root: true,
        })
    }
}

/// Every fix, in a stable order.
pub fn all() -> Vec<Box<dyn Fix>> {
    vec![
        Box::new(AptClean),
        Box::new(JournalVacuum),
        Box::new(AutoremoveKernels),
        Box::new(SnapRetain),
        Box::new(FlatpakRemoveUnused),
        Box::new(EnableUfw),
    ]
}

/// Look up a fix by id.
pub fn find(id: &str) -> Option<Box<dyn Fix>> {
    all().into_iter().find(|f| f.id() == id)
}

/// Every fix id, for validation in the privileged helper.
pub const FIX_IDS: &[&str] = &[
    "fix.apt_clean",
    "fix.journal_vacuum",
    "fix.autoremove",
    "fix.snap_retain",
    "fix.flatpak_unused",
    "fix.enable_ufw",
];

/// The fix that resolves a given finding, if one exists. Lets the UI put an
/// "Apply fix" button on the findings it can remedy.
pub fn fix_for_finding(finding_id: &str) -> Option<&'static str> {
    match finding_id {
        "packages.apt_cache_large" => Some("fix.apt_clean"),
        "logs.journal_large" => Some("fix.journal_vacuum"),
        "packages.old_kernels" => Some("fix.autoremove"),
        "snap.old_revisions" => Some("fix.snap_retain"),
        "flatpak.unused_runtimes" => Some("fix.flatpak_unused"),
        "security.firewall_inactive" => Some("fix.enable_ufw"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sysmedic_core::snapshot::*;

    #[test]
    fn registry_and_ids_agree() {
        let ids: Vec<&str> = all().iter().map(|f| f.id()).collect();
        assert_eq!(ids, FIX_IDS);
        for id in FIX_IDS {
            assert!(find(id).is_some(), "no fix for {id}");
        }
        assert!(find("fix.nonexistent").is_none());
    }

    #[test]
    fn finding_to_fix_mapping_targets_real_fixes() {
        for finding in [
            "packages.apt_cache_large",
            "logs.journal_large",
            "packages.old_kernels",
            "snap.old_revisions",
            "flatpak.unused_runtimes",
            "security.firewall_inactive",
        ] {
            let fix_id = fix_for_finding(finding).expect("mapping exists");
            assert!(FIX_IDS.contains(&fix_id), "{fix_id} not a real fix");
        }
        assert!(fix_for_finding("cpu.high_load").is_none());
    }

    #[test]
    fn fixes_are_quiet_on_a_clean_system() {
        let s = Snapshot::default();
        for fix in all() {
            assert!(fix.plan(&s).is_none(), "{} fired on clean system", fix.id());
        }
    }

    #[test]
    fn apt_clean_applies_above_threshold() {
        let s = Snapshot {
            packages: Some(PackageInfo {
                apt_cache_bytes: Some(2 * 1024 * 1024 * 1024),
                ..Default::default()
            }),
            ..Default::default()
        };
        let plan = AptClean.plan(&s).unwrap();
        assert_eq!(plan.commands[0].display(), "apt-get clean");
        assert!(!plan.reversible);
    }

    #[test]
    fn enable_ufw_is_reversible() {
        let mut s = Snapshot {
            security: Some(SecurityInfo {
                firewall_active: Some(false),
                ssh_permit_root_login: None,
            }),
            ..Default::default()
        };
        let plan = EnableUfw.plan(&s).unwrap();
        assert!(plan.reversible);
        assert_eq!(plan.undo[0].display(), "ufw disable");
        // Not applicable once the firewall is active.
        s.security.as_mut().unwrap().firewall_active = Some(true);
        assert!(EnableUfw.plan(&s).is_none());
    }

    #[test]
    fn autoremove_needs_more_than_two_old_kernels() {
        let mut s = Snapshot {
            packages: Some(PackageInfo {
                old_kernels: vec!["a".into(), "b".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(AutoremoveKernels.plan(&s).is_none());
        s.packages.as_mut().unwrap().old_kernels.push("c".into());
        assert_eq!(AutoremoveKernels.plan(&s).unwrap().risk, Severity::Medium);
    }
}
