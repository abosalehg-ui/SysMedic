//! The concrete fixes. Each turns a [`Snapshot`] into a [`FixPlan`] (or
//! `None` when not applicable). Plans are pure data — building one runs no
//! commands — so every fix is unit-tested against fixture snapshots.

use once_cell::sync::Lazy;
use sysmedic_core::fix::{FixCommand, FixPlan};
use sysmedic_core::lang::LocalizedText;
use sysmedic_core::{thresholds, Severity, Snapshot};

/// One safe fix SysMedic can offer.
pub trait Fix: Send + Sync {
    /// Stable id, e.g. `fix.apt_clean`.
    fn id(&self) -> &'static str;
    /// Short bilingual title. Snapshot-independent, so `undo` can name the
    /// fix it is reversing in the user's language from the journal's fix id
    /// alone, without rebuilding a plan.
    fn title(&self) -> LocalizedText;
    /// Build the plan for this system, or `None` if there is nothing to do.
    fn plan(&self, snapshot: &Snapshot) -> Option<FixPlan>;
    /// The commands that reverse this fix, or empty if it is irreversible.
    ///
    /// This is the single source of truth for undo: it is compiled in and
    /// snapshot-independent, so the privileged helper can rebuild it from the
    /// fix id alone rather than trusting commands read back from the journal
    /// file (which an attacker who could write that file might tamper with).
    fn undo(&self) -> Vec<FixCommand> {
        Vec::new()
    }
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

pub struct AptClean;
impl Fix for AptClean {
    fn id(&self) -> &'static str {
        "fix.apt_clean"
    }
    fn title(&self) -> LocalizedText {
        LocalizedText::new("Clear the APT download cache", "تفريغ ذاكرة تنزيل APT")
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        let bytes = s.packages.as_ref()?.apt_cache_bytes?;
        if bytes < thresholds::packages::APT_CACHE_FIX_BYTES {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: self.title(),
            description: LocalizedText::new(
                format!(
                    "Delete cached .deb files in /var/cache/apt/archives, freeing about \
                     {:.1} GiB. Packages re-download on demand if needed.",
                    gib(bytes)
                ),
                format!(
                    "حذف ملفات .deb المخزّنة في ‎/var/cache/apt/archives، ما يحرّر نحو \
                     {:.1} غيغابايت. تُعاد الحزم تنزيلاً عند الحاجة إليها.",
                    gib(bytes)
                ),
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
    fn title(&self) -> LocalizedText {
        LocalizedText::new("Trim the systemd journal", "تقليص سجلّ systemd")
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        let bytes = s.logs.as_ref()?.journal_bytes?;
        if bytes < thresholds::journal::LARGE_BYTES {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: self.title(),
            description: LocalizedText::new(
                format!(
                    "The journal currently uses {:.1} GiB. This keeps the most recent 200 MB \
                     of logs and deletes older archived entries.",
                    gib(bytes)
                ),
                format!(
                    "يستهلك السجلّ حالياً {:.1} غيغابايت. سيُبقي هذا الإجراء أحدث 200 ميغابايت \
                     من السجلّات ويحذف المدخلات المؤرشفة الأقدم.",
                    gib(bytes)
                ),
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
    fn title(&self) -> LocalizedText {
        LocalizedText::new(
            "Remove old kernels and orphaned packages",
            "إزالة النوى القديمة والحزم اليتيمة",
        )
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        let kernels = &s.packages.as_ref()?.old_kernels;
        if kernels.len() <= thresholds::packages::OLD_KERNELS_KEPT {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: self.title(),
            description: LocalizedText::new(
                format!(
                    "Purge {} old kernel image(s) and any auto-installed packages no longer \
                     needed. The running kernel and one fallback are always kept.",
                    kernels.len()
                ),
                format!(
                    "إزالة {} من صور النواة القديمة وأي حزم مُثبَّتة تلقائياً لم تعد لازمة. \
                     تُحفَظ دائماً النواة العاملة ونواة احتياطية واحدة.",
                    kernels.len()
                ),
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
    fn title(&self) -> LocalizedText {
        LocalizedText::new(
            "Keep fewer old snap revisions",
            "الإبقاء على عدد أقل من نسخ snap القديمة",
        )
    }
    fn undo(&self) -> Vec<FixCommand> {
        vec![FixCommand::new(
            "snap",
            &["set", "system", "refresh.retain=3"],
        )]
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        let snap = s.snap.as_ref()?;
        if snap.disabled_revisions == 0 {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: self.title(),
            description: LocalizedText::new(
                format!(
                    "Set snapd to retain only 2 revisions per snap (currently {} disabled \
                     revision(s) are held). snapd prunes the extras on the next refresh.",
                    snap.disabled_revisions
                ),
                format!(
                    "ضبط snapd للاحتفاظ بنسختين فقط لكل snap (يُحتفظ حالياً بـ {} نسخة \
                     معطّلة). يحذف snapd الزائد عند التحديث التالي.",
                    snap.disabled_revisions
                ),
            ),
            commands: vec![FixCommand::new(
                "snap",
                &["set", "system", "refresh.retain=2"],
            )],
            affected_paths: vec!["/var/lib/snapd/snaps".into()],
            reversible: true,
            undo: self.undo(),
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
    fn title(&self) -> LocalizedText {
        LocalizedText::new(
            "Remove unused Flatpak runtimes",
            "إزالة بيئات تشغيل Flatpak غير المستخدمة",
        )
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        let refs = &s.flatpak.as_ref()?.unused_refs;
        if refs.is_empty() {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: self.title(),
            description: LocalizedText::new(
                format!(
                    "Uninstall {} Flatpak runtime(s) that no installed app needs.",
                    refs.len()
                ),
                format!(
                    "إلغاء تثبيت {} من بيئات تشغيل Flatpak لا يحتاجها أي تطبيق مُثبَّت.",
                    refs.len()
                ),
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
    fn title(&self) -> LocalizedText {
        LocalizedText::new("Enable the firewall", "تفعيل الجدار الناري")
    }
    fn undo(&self) -> Vec<FixCommand> {
        vec![FixCommand::new("ufw", &["disable"])]
    }
    fn plan(&self, s: &Snapshot) -> Option<FixPlan> {
        if s.security.as_ref()?.firewall_active != Some(false) {
            return None;
        }
        Some(FixPlan {
            id: self.id().into(),
            title: self.title(),
            description: LocalizedText::new(
                "Turn on ufw with its default policy: deny incoming, allow outgoing. \
                 Suitable for a desktop with no server software.",
                "تشغيل ufw بسياسته الافتراضية: منع الوارد، والسماح بالصادر. مناسب لجهاز \
                 مكتبي لا يشغّل برمجيات خادم.",
            ),
            commands: vec![FixCommand::new("ufw", &["--force", "enable"])],
            affected_paths: vec!["/etc/ufw".into(), "/lib/systemd/system/ufw.service".into()],
            reversible: true,
            undo: self.undo(),
            risk: Severity::Low,
            needs_root: true,
        })
    }
}

/// The registry, built once. Previously `all()` boxed six trait objects on
/// every call, and `find`/`undo_commands` went through it — so simply
/// rendering the findings list rebuilt the whole registry per finding.
static REGISTRY: Lazy<Vec<Box<dyn Fix>>> = Lazy::new(|| {
    vec![
        Box::new(AptClean) as Box<dyn Fix>,
        Box::new(JournalVacuum),
        Box::new(AutoremoveKernels),
        Box::new(SnapRetain),
        Box::new(FlatpakRemoveUnused),
        Box::new(EnableUfw),
    ]
});

/// Every fix, in a stable order.
pub fn all() -> &'static [Box<dyn Fix>] {
    &REGISTRY
}

/// Look up a fix by id.
pub fn find(id: &str) -> Option<&'static dyn Fix> {
    REGISTRY.iter().find(|f| f.id() == id).map(|b| b.as_ref())
}

/// The compiled-in undo commands for a fix id, or `None` if the id is unknown.
/// A known but irreversible fix yields `Some(vec![])`.
///
/// `undo` uses this instead of the commands stored in the journal, so the
/// privileged helper trusts only the fix id — mirroring how `apply` rebuilds
/// the plan from the id rather than accepting commands from its caller.
pub fn undo_commands(id: &str) -> Option<Vec<FixCommand>> {
    Some(find(id)?.undo())
}

/// Every fix id, for validation in the privileged helper.
///
/// **Derived** from the registry rather than written out by hand. The previous
/// hand-maintained list was a second source of truth for the set the helper
/// validates against before doing anything as root; a fix added to `all()` but
/// forgotten here would have been rejected, and — worse — an id left here
/// after its fix was removed would have been accepted.
static FIX_ID_LIST: Lazy<Vec<&'static str>> =
    Lazy::new(|| REGISTRY.iter().map(|f| f.id()).collect());

/// Every fix id the helper will accept.
pub fn fix_ids() -> &'static [&'static str] {
    &FIX_ID_LIST
}

/// The fix that resolves a given finding, if one exists. Lets the UI put an
/// "Apply fix" button on the findings it can remedy.
pub fn fix_for_finding(finding_id: &str) -> Option<&'static str> {
    match finding_id {
        "packages.apt_cache_large" => Some("fix.apt_clean"),
        "logs.journal_large" => Some("fix.journal_vacuum"),
        "packages.old_kernels" => Some("fix.autoremove"),
        "storage.snap_old_revisions" => Some("fix.snap_retain"),
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
        assert_eq!(ids, fix_ids());
        for id in fix_ids() {
            assert!(find(id).is_some(), "no fix for {id}");
        }
        assert!(find("fix.nonexistent").is_none());
    }

    #[test]
    fn fix_ids_are_unique() {
        // Two fixes sharing an id would make `find` (and therefore the
        // helper's undo path) resolve to whichever came first.
        let mut ids: Vec<&str> = fix_ids().to_vec();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate fix id in the registry");
    }

    #[test]
    fn undo_commands_come_from_the_registry() {
        assert_eq!(
            undo_commands("fix.enable_ufw").unwrap()[0].display(),
            "ufw disable"
        );
        assert_eq!(
            undo_commands("fix.snap_retain").unwrap()[0].display(),
            "snap set system refresh.retain=3"
        );
        // Known but irreversible → empty; unknown id → None (helper refuses it).
        assert!(undo_commands("fix.apt_clean").unwrap().is_empty());
        assert!(undo_commands("fix.nonexistent").is_none());
    }

    #[test]
    fn finding_to_fix_mapping_targets_real_fixes() {
        for finding in [
            "packages.apt_cache_large",
            "logs.journal_large",
            "packages.old_kernels",
            "storage.snap_old_revisions",
            "flatpak.unused_runtimes",
            "security.firewall_inactive",
        ] {
            let fix_id = fix_for_finding(finding).expect("mapping exists");
            assert!(fix_ids().contains(&fix_id), "{fix_id} not a real fix");
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
                manager: Some("apt".into()),
                pacman_cache_bytes: None,
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
                ssh_password_auth: None,
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
                manager: Some("apt".into()),
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
