//! The concrete fixes. Each turns a [`Snapshot`] into a [`FixPlan`] (or
//! `None` when not applicable). Plans are pure data — building one runs no
//! commands — so every fix is unit-tested against fixture snapshots.

use once_cell::sync::Lazy;
use sysmedic_core::fix::{FixCommand, FixPlan};
use sysmedic_core::lang::LocalizedText;
use sysmedic_core::{thresholds, Severity, Snapshot};

/// Which polkit action authorizes a fix.
///
/// pkexec picks its action from the *path of the program it launches*, so a
/// single helper binary can only ever map to a single action — and a single
/// action means one generic prompt ("Apply a SysMedic system fix") for both
/// "empty the download cache" and "purge packages and their configuration".
/// Splitting the helper in two gives each class its own action, so the polkit
/// dialog describes what is actually about to happen and an administrator can
/// write policy for one class without the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixTier {
    /// Undoable, and touches only a setting: the firewall switch, snapd's
    /// retention count.
    Routine,
    /// Deletes something that cannot be brought back by `undo` — cached
    /// packages, log history, installed packages and their configuration.
    Destructive,
}

impl FixTier {
    /// The tier a plan belongs to. This is the definition; [`Fix::tier`]
    /// declares it per fix and a test holds the two together.
    pub fn of(plan: &FixPlan) -> FixTier {
        if plan.reversible && plan.risk < Severity::Medium {
            FixTier::Routine
        } else {
            FixTier::Destructive
        }
    }
}

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

    /// Which privileged helper — and therefore which polkit action — may run
    /// this fix.
    ///
    /// Declared per fix rather than derived from a plan, because the caller
    /// has to choose the binary to launch *before* any plan exists. Defaults
    /// to [`FixTier::Destructive`] so a newly added fix gets the stricter
    /// prompt until someone deliberately says otherwise; a test checks each
    /// declaration against the fix's actual plan.
    fn tier(&self) -> FixTier {
        FixTier::Destructive
    }
    /// The commands that reverse this fix, or empty if it is irreversible.
    ///
    /// This is the single source of truth for undo: it is compiled in and
    /// snapshot-independent, so the privileged helper can rebuild it from the
    /// fix id alone rather than trusting commands read back from the journal
    /// file (which an attacker who could write that file might tamper with).
    fn undo(&self) -> Vec<FixCommand> {
        Vec::new()
    }

    /// [`Fix::undo`], given the value this fix overwrote (as recorded in the
    /// journal when the fix was applied).
    ///
    /// Only fixes that *replace* a setting need this; the default ignores the
    /// argument. The program and its options stay compiled in — an
    /// implementation may substitute `restore` into an argument, but must
    /// validate it first, because the journal is a file on disk and this runs
    /// as root.
    fn undo_from(&self, _restore: Option<&str>) -> Vec<FixCommand> {
        self.undo()
    }
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

/// How many package names a preview lists before summarising the rest. Enough
/// to recognise what is going, short enough to stay readable in a dialog.
const PREVIEW_PACKAGE_LIMIT: usize = 12;

/// A readable package list for a fix preview, truncated with a count.
fn package_list(packages: &[String]) -> String {
    if packages.len() <= PREVIEW_PACKAGE_LIMIT {
        return packages.join(", ");
    }
    format!(
        "{}, … (+{})",
        packages[..PREVIEW_PACKAGE_LIMIT].join(", "),
        packages.len() - PREVIEW_PACKAGE_LIMIT
    )
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
            restore: None,
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
            restore: None,
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
        let packages = s.packages.as_ref()?;
        let kernels = &packages.old_kernels;
        if kernels.len() <= thresholds::packages::OLD_KERNELS_KEPT {
            return None;
        }
        // The preview is the consent contract for an irreversible privileged
        // change, so it must name what will actually be removed. The old text
        // spoke only of kernels while `autoremove --purge` deletes every
        // auto-installed package nothing needs any more — and `--purge` takes
        // their /etc configuration with them. The list comes from an
        // unprivileged `apt-get -s` simulation run by the collector.
        let removed = &packages.autoremovable;
        let (extra_en, extra_ar) = if removed.is_empty() {
            (String::new(), String::new())
        } else {
            (
                format!(
                    "\n\nIt will remove {} package(s) in total: {}.",
                    removed.len(),
                    package_list(removed)
                ),
                format!(
                    "\n\nسيزيل {} حزمة إجمالاً: {}.",
                    removed.len(),
                    package_list(removed)
                ),
            )
        };
        Some(FixPlan {
            id: self.id().into(),
            title: self.title(),
            description: LocalizedText::new(
                format!(
                    "Purge {} old kernel image(s) and any auto-installed packages no longer \
                     needed, including their configuration files in /etc. The running kernel \
                     and one fallback are always kept.{extra_en}",
                    kernels.len()
                ),
                format!(
                    "إزالة {} من صور النواة القديمة وأي حزم مُثبَّتة تلقائياً لم تعد لازمة، \
                     بما في ذلك ملفات إعداداتها في ‎/etc. تُحفَظ دائماً النواة العاملة ونواة \
                     احتياطية واحدة.{extra_ar}",
                    kernels.len()
                ),
            ),
            commands: vec![FixCommand::new("apt-get", &["autoremove", "--purge", "-y"])],
            // `/etc` is listed because `--purge` deletes the removed packages'
            // configuration there; omitting it understated the blast radius.
            affected_paths: vec!["/boot".into(), "/lib/modules".into(), "/etc".into()],
            reversible: false,
            undo: vec![],
            restore: None,
            risk: Severity::Medium,
            needs_root: true,
        })
    }
}

/// snapd's own default for `refresh.retain`, and the range it accepts. Used
/// both as the fallback when we never learned the user's value and as the
/// validation bound for a value read back from the journal.
const SNAPD_DEFAULT_RETAIN: u32 = 3;
const MIN_RETAIN: u32 = 2;
const MAX_RETAIN: u32 = 20;
/// What the fix sets: keeping two revisions is the smallest snapd allows,
/// i.e. the current one plus a rollback.
const TARGET_RETAIN: u32 = 2;

pub struct SnapRetain;

impl SnapRetain {
    fn set_retain(value: u32) -> Vec<FixCommand> {
        vec![FixCommand::new(
            "snap",
            &["set", "system", &format!("refresh.retain={value}")],
        )]
    }
}

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
    fn tier(&self) -> FixTier {
        FixTier::Routine
    }
    /// Without a recorded previous value, fall back to snapd's own default.
    fn undo(&self) -> Vec<FixCommand> {
        Self::set_retain(SNAPD_DEFAULT_RETAIN)
    }

    /// Restore the value the system actually had before the fix.
    ///
    /// The old `undo` hardcoded `refresh.retain=3` — snapd's default, not the
    /// user's setting. Someone running `retain=10` who applied this fix and
    /// then pressed undo silently ended up at 3 and believed they were back
    /// where they started, which makes "reversible: yes" in the consent dialog
    /// untrue.
    ///
    /// `restore` comes from the journal file, so it is validated before use:
    /// only an integer in snapd's accepted range is substituted, and anything
    /// else falls back to the default. The program and option name are
    /// compiled in, so the worst a tampered journal can do is choose a
    /// different (valid) retention count.
    fn undo_from(&self, restore: Option<&str>) -> Vec<FixCommand> {
        let value = restore
            .and_then(|v| v.trim().parse::<u32>().ok())
            .filter(|v| (MIN_RETAIN..=MAX_RETAIN).contains(v))
            .unwrap_or(SNAPD_DEFAULT_RETAIN);
        Self::set_retain(value)
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
                    "Set snapd to retain only {TARGET_RETAIN} revisions per snap (currently {} \
                     disabled revision(s) are held{}). snapd prunes the extras on the next \
                     refresh.",
                    snap.disabled_revisions,
                    snap.refresh_retain
                        .map(|r| format!(", and refresh.retain is {r}"))
                        .unwrap_or_default()
                ),
                format!(
                    "ضبط snapd للاحتفاظ بـ {TARGET_RETAIN} نسختين فقط لكل snap (يُحتفظ حالياً بـ {} \
                     نسخة معطّلة{}). يحذف snapd الزائد عند التحديث التالي.",
                    snap.disabled_revisions,
                    snap.refresh_retain
                        .map(|r| format!("، وقيمة refresh.retain الحالية {r}"))
                        .unwrap_or_default()
                ),
            ),
            commands: Self::set_retain(TARGET_RETAIN),
            affected_paths: vec!["/var/lib/snapd/snaps".into()],
            reversible: true,
            // Undo must restore the value this system had, so carry it into
            // the journal rather than assuming snapd's default.
            undo: self.undo_from(snap.refresh_retain.map(|r| r.to_string()).as_deref()),
            restore: snap.refresh_retain.map(|r| r.to_string()),
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
            restore: None,
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
    fn tier(&self) -> FixTier {
        FixTier::Routine
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
            restore: None,
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

/// The fix ids a helper of `tier` is allowed to run.
///
/// Each privileged binary validates against its own subset, so the routine
/// helper physically cannot purge packages even if asked — the split is
/// enforced in code, not only in the polkit prompt.
pub fn fix_ids_for(tier: FixTier) -> Vec<&'static str> {
    REGISTRY
        .iter()
        .filter(|f| f.tier() == tier)
        .map(|f| f.id())
        .collect()
}

/// The tier of a fix id, or `None` when the id is unknown.
pub fn tier_of(id: &str) -> Option<FixTier> {
    Some(find(id)?.tier())
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

    /// A snapshot on which every fix in the registry applies.
    pub(crate) fn unhealthy() -> Snapshot {
        Snapshot {
            packages: Some(PackageInfo {
                manager: Some("apt".into()),
                apt_cache_bytes: Some(2 * 1024 * 1024 * 1024),
                old_kernels: vec!["a".into(), "b".into(), "c".into()],
                autoremovable: vec!["linux-image-a".into()],
                ..Default::default()
            }),
            logs: Some(LogInfo {
                journal_bytes: Some(5 * 1024 * 1024 * 1024),
                large_files: vec![],
            }),
            snap: Some(SnapInfo {
                disabled_revisions: 3,
                snaps_dir_bytes: None,
                refresh_retain: Some(5),
            }),
            flatpak: Some(FlatpakInfo {
                unused_refs: vec!["runtime/org.freedesktop.Platform/x86_64/23.08".into()],
            }),
            security: Some(SecurityInfo {
                firewall_active: Some(false),
                ssh_permit_root_login: None,
                ssh_password_auth: None,
            }),
            ..Default::default()
        }
    }

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
    fn declared_tiers_match_the_plans_they_produce() {
        // The declaration drives which privileged binary runs a fix, and the
        // plan drives what the user is shown. A fix whose plan turned
        // irreversible while its declaration still said "routine" would be
        // authorized by the gentler prompt.
        let s = unhealthy();
        for fix in all() {
            let plan = fix
                .plan(&s)
                .unwrap_or_else(|| panic!("{} did not apply", fix.id()));
            assert_eq!(
                fix.tier(),
                FixTier::of(&plan),
                "{} declares {:?} but its plan is reversible={} risk={:?}",
                fix.id(),
                fix.tier(),
                plan.reversible,
                plan.risk
            );
        }
    }

    #[test]
    fn the_two_tiers_partition_the_registry() {
        let routine = fix_ids_for(FixTier::Routine);
        let destructive = fix_ids_for(FixTier::Destructive);
        assert_eq!(routine.len() + destructive.len(), fix_ids().len());
        assert!(routine.iter().all(|id| !destructive.contains(id)));
        // The destructive set is the one that removes things.
        assert!(destructive.contains(&"fix.autoremove"));
        assert!(destructive.contains(&"fix.apt_clean"));
        assert!(routine.contains(&"fix.enable_ufw"));
        assert_eq!(tier_of("fix.enable_ufw"), Some(FixTier::Routine));
        assert_eq!(tier_of("fix.autoremove"), Some(FixTier::Destructive));
        assert_eq!(tier_of("fix.nope"), None);
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
    fn snap_retain_undo_restores_the_users_value() {
        let s = Snapshot {
            snap: Some(SnapInfo {
                disabled_revisions: 4,
                snaps_dir_bytes: None,
                refresh_retain: Some(10),
            }),
            ..Default::default()
        };
        let plan = SnapRetain.plan(&s).unwrap();
        assert_eq!(
            plan.commands[0].display(),
            "snap set system refresh.retain=2"
        );
        // Undo puts back 10, not snapd's default of 3 — otherwise "reversible"
        // quietly means "reset to a value you never chose".
        assert_eq!(plan.undo[0].display(), "snap set system refresh.retain=10");
        assert_eq!(plan.restore.as_deref(), Some("10"));
        // The current value is named in the consent text.
        assert!(plan.description.en.contains("refresh.retain is 10"));
    }

    #[test]
    fn snap_retain_falls_back_to_the_default_when_the_value_is_unknown() {
        let s = Snapshot {
            snap: Some(SnapInfo {
                disabled_revisions: 4,
                snaps_dir_bytes: None,
                refresh_retain: None,
            }),
            ..Default::default()
        };
        let plan = SnapRetain.plan(&s).unwrap();
        assert_eq!(plan.undo[0].display(), "snap set system refresh.retain=3");
        assert!(plan.restore.is_none());
    }

    #[test]
    fn snap_retain_validates_the_value_read_back_from_the_journal() {
        // `restore` comes from a file on disk and is used by a root process,
        // so anything that is not a plausible retention count is ignored.
        for hostile in [
            "3; rm -rf /",
            "$(id)",
            "-1",
            "0",
            "999",
            "",
            "../../etc/passwd",
        ] {
            let commands = SnapRetain.undo_from(Some(hostile));
            assert_eq!(
                commands[0].display(),
                "snap set system refresh.retain=3",
                "hostile value {hostile:?} was not rejected"
            );
        }
        // A legitimate value is honoured.
        assert_eq!(
            SnapRetain.undo_from(Some("7"))[0].display(),
            "snap set system refresh.retain=7"
        );
    }

    #[test]
    fn autoremove_preview_names_what_it_actually_removes() {
        // The old preview spoke only of kernels while `--purge` also deletes
        // unrelated auto-installed packages and their /etc configuration.
        let s = Snapshot {
            packages: Some(PackageInfo {
                manager: Some("apt".into()),
                old_kernels: vec!["a".into(), "b".into(), "c".into()],
                autoremovable: vec![
                    "linux-image-6.8.0-45-generic".into(),
                    "libpython3.11-minimal".into(),
                    "nodejs-doc".into(),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let plan = AutoremoveKernels.plan(&s).unwrap();
        assert!(
            plan.description.en.contains("nodejs-doc"),
            "{:?}",
            plan.description.en
        );
        assert!(plan.description.en.contains("3 package(s) in total"));
        assert!(plan.description.ar.contains("nodejs-doc"));
        assert!(plan.description.en.contains("/etc"));
        // And the affected-paths list no longer understates the blast radius.
        assert!(plan.affected_paths.iter().any(|p| p == "/etc"));
    }

    #[test]
    fn autoremove_preview_truncates_a_long_package_list() {
        let many: Vec<String> = (0..30).map(|i| format!("pkg{i}")).collect();
        let listed = package_list(&many);
        assert!(listed.contains("pkg0") && listed.contains("pkg11"));
        assert!(listed.contains("(+18)"), "got: {listed}");
        assert!(!listed.contains("pkg12"));
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
