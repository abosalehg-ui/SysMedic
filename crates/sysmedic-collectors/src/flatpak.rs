use sysmedic_core::snapshot::FlatpakInfo;
use sysmedic_core::{Collector, Snapshot};

use crate::util;

pub struct FlatpakCollector;

impl Collector for FlatpakCollector {
    fn name(&self) -> &'static str {
        "flatpak"
    }

    fn collect(&self, snapshot: &mut Snapshot) {
        // `flatpak uninstall --unused` lists (and would remove) runtimes no
        // installed app needs. We only *list* here, never remove.
        let Some(out) = util::run(
            "flatpak",
            &["list", "--columns=application,ref", "--runtime"],
        ) else {
            return; // flatpak not installed
        };
        let unused = util::run("flatpak", &["list", "--app", "--columns=runtime"])
            .map(|apps| unused_refs(&out, &apps))
            .unwrap_or_default();
        snapshot.flatpak = Some(FlatpakInfo {
            unused_refs: unused,
        });
    }
}

/// A runtime ref is unused when no installed application declares it as its
/// runtime. `runtimes` is the `application,ref` listing of installed
/// runtimes; `app_runtimes` is the `runtime` column of installed apps.
pub fn unused_refs(runtimes: &str, app_runtimes: &str) -> Vec<String> {
    let needed: Vec<&str> = app_runtimes
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    runtimes
        .lines()
        .filter_map(|line| {
            let mut cols = line.split_whitespace();
            let app = cols.next()?;
            let reference = cols.next().unwrap_or(app);
            // A runtime is needed if some app's runtime column matches its
            // application id or full ref.
            let is_needed = needed
                .iter()
                .any(|r| r.contains(app) || reference.contains(*r) || *r == reference);
            (!is_needed).then(|| reference.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_unused_runtime() {
        let runtimes = "org.freedesktop.Platform runtime/org.freedesktop.Platform/x86_64/23.08\norg.gnome.Platform runtime/org.gnome.Platform/x86_64/45\n";
        let app_runtimes = "org.gnome.Platform/x86_64/45\n";
        let unused = unused_refs(runtimes, app_runtimes);
        assert_eq!(
            unused,
            vec!["runtime/org.freedesktop.Platform/x86_64/23.08"]
        );
    }

    #[test]
    fn nothing_unused_when_all_referenced() {
        let runtimes = "org.gnome.Platform runtime/org.gnome.Platform/x86_64/45\n";
        let app_runtimes = "org.gnome.Platform/x86_64/45\n";
        assert!(unused_refs(runtimes, app_runtimes).is_empty());
    }
}
