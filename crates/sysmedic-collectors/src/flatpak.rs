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
        let Some(runtimes) = util::run(
            "flatpak",
            &["list", "--columns=application,ref", "--runtime"],
        ) else {
            return; // flatpak not installed
        };
        // The app id as well as its runtime: an app's own extensions
        // (`org.mozilla.firefox.Locale`) are listed among the runtimes and
        // are only recognisable by the app id they are named after.
        let unused = util::run(
            "flatpak",
            &["list", "--app", "--columns=application,runtime"],
        )
        .map(|apps| unused_refs(&runtimes, &apps))
        .unwrap_or_default();
        snapshot.flatpak = Some(FlatpakInfo {
            unused_refs: unused,
        });
    }
}

/// A parsed `id/arch/branch` ref.
struct Ref<'a> {
    id: &'a str,
    branch: Option<&'a str>,
}

fn parse_ref(reference: &str) -> Ref<'_> {
    let reference = reference.strip_prefix("runtime/").unwrap_or(reference);
    let mut parts = reference.split('/');
    Ref {
        id: parts.next().unwrap_or(reference),
        branch: parts.nth(1),
    }
}

/// Runtime refs no installed application needs.
///
/// `runtimes` is the `application,ref` listing of installed runtimes;
/// `apps` is the `application,runtime` listing of installed apps.
///
/// `flatpak list --runtime` lists **extensions** too — `org.gnome.Platform.Locale`,
/// `org.freedesktop.Platform.GL.default`, `….openh264`, `org.gtk.Gtk3theme.*`,
/// an app's own `.Locale` — and no app names any of those in its `runtime`
/// column. Matching that column literally therefore reported four to ten
/// "unused runtimes" on every desktop with a single Flatpak app, and the
/// consent dialog for `fix.flatpak_unused` listed them as what would be
/// removed, while `flatpak uninstall --unused` (which resolves the real
/// dependency graph) would have touched few or none of them.
///
/// An extension is named after the thing it extends: its id is its parent's
/// id plus a dotted suffix. So a runtime ref counts as needed when:
/// * some app's runtime column names exactly this ref (the plain case);
/// * its id extends a needed runtime's id, whatever its branch — extensions
///   version independently (`openh264/2.4.1` extends `Platform/24.08`), so
///   the branch cannot tell a current one from a stale one;
/// * its id extends an installed app's id.
///
/// This errs towards "needed": a stale `GL.default/23.08` beside a used
/// `Platform/24.08` is not reported, but a `Locale` for an installed app is
/// never offered for removal in a dialog that promises nothing needed goes.
/// The fix command itself (`flatpak uninstall --unused`) resolves the real
/// graph and removes the stale one anyway.
pub fn unused_refs(runtimes: &str, apps: &str) -> Vec<String> {
    let mut needed_runtimes: Vec<Ref<'_>> = Vec::new();
    let mut app_ids: Vec<&str> = Vec::new();
    for line in apps.lines() {
        let mut cols = line.split_whitespace();
        let Some(app) = cols.next() else { continue };
        app_ids.push(app);
        if let Some(runtime) = cols.next() {
            needed_runtimes.push(parse_ref(runtime));
        }
    }

    runtimes
        .lines()
        .filter_map(|line| {
            let mut cols = line.split_whitespace();
            let app = cols.next()?;
            let reference = cols.next().unwrap_or(app);
            let candidate = parse_ref(reference);

            let exact = needed_runtimes.iter().any(|needed| {
                needed.id == candidate.id
                    && (needed.branch.is_none() || needed.branch == candidate.branch)
            });
            let extends_runtime = needed_runtimes
                .iter()
                .any(|needed| extends(candidate.id, needed.id));
            let extends_app = app_ids.iter().any(|id| extends(candidate.id, id));

            (!(exact || extends_runtime || extends_app)).then(|| reference.to_string())
        })
        .collect()
}

/// Whether `id` names an extension of `parent` (`parent.Something…`).
fn extends(id: &str, parent: &str) -> bool {
    id.strip_prefix(parent)
        .is_some_and(|rest| rest.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_unused_runtime() {
        let runtimes = "org.freedesktop.Platform runtime/org.freedesktop.Platform/x86_64/23.08\norg.gnome.Platform runtime/org.gnome.Platform/x86_64/45\n";
        let apps = "org.gnome.Calculator org.gnome.Platform/x86_64/45\n";
        let unused = unused_refs(runtimes, apps);
        assert_eq!(
            unused,
            vec!["runtime/org.freedesktop.Platform/x86_64/23.08"]
        );
    }

    #[test]
    fn nothing_unused_when_all_referenced() {
        let runtimes = "org.gnome.Platform runtime/org.gnome.Platform/x86_64/45\n";
        let apps = "org.gnome.Calculator org.gnome.Platform/x86_64/45\n";
        assert!(unused_refs(runtimes, apps).is_empty());
    }

    #[test]
    fn extensions_of_needed_runtimes_and_apps_are_not_unused() {
        // A real desktop with one GNOME app and Firefox: the runtime listing
        // is mostly extensions, none of which any app names as its runtime.
        // The old literal match reported every one of them.
        let runtimes = "\
org.gnome.Platform runtime/org.gnome.Platform/x86_64/47
org.gnome.Platform.Locale runtime/org.gnome.Platform.Locale/x86_64/47
org.freedesktop.Platform runtime/org.freedesktop.Platform/x86_64/24.08
org.freedesktop.Platform.GL.default runtime/org.freedesktop.Platform.GL.default/x86_64/24.08
org.freedesktop.Platform.GL.default runtime/org.freedesktop.Platform.GL.default/x86_64/24.08extra
org.freedesktop.Platform.openh264 runtime/org.freedesktop.Platform.openh264/x86_64/2.4.1
org.freedesktop.Platform.VAAPI.Intel runtime/org.freedesktop.Platform.VAAPI.Intel/x86_64/24.08
org.mozilla.firefox.Locale runtime/org.mozilla.firefox.Locale/x86_64/stable
org.gtk.Gtk3theme.Yaru runtime/org.gtk.Gtk3theme.Yaru/x86_64/3.22
";
        let apps = "\
org.gnome.Calculator org.gnome.Platform/x86_64/47
org.mozilla.firefox org.freedesktop.Platform/x86_64/24.08
";
        // The only genuinely unmatched ref is the GTK theme, which belongs to
        // no runtime and no app by name.
        assert_eq!(
            unused_refs(runtimes, apps),
            vec!["runtime/org.gtk.Gtk3theme.Yaru/x86_64/3.22"]
        );
    }

    #[test]
    fn an_extension_of_a_needed_runtime_is_kept_whatever_its_branch() {
        // Extensions version on their own: openh264 is 2.4.1 under a 24.08
        // platform. A branch comparison would have reported it. The price is
        // that a genuinely stale GL.default/23.08 is not reported either —
        // which is the safe direction for a consent dialog, and the fix
        // command resolves the real graph anyway.
        let runtimes = "\
org.freedesktop.Platform runtime/org.freedesktop.Platform/x86_64/24.08
org.freedesktop.Platform.openh264 runtime/org.freedesktop.Platform.openh264/x86_64/2.4.1
org.freedesktop.Platform.GL.default runtime/org.freedesktop.Platform.GL.default/x86_64/23.08
";
        let apps = "org.mozilla.firefox org.freedesktop.Platform/x86_64/24.08\n";
        assert!(unused_refs(runtimes, apps).is_empty());

        // A whole *runtime* on a stale branch, by contrast, is reported.
        let runtimes = "\
org.freedesktop.Platform runtime/org.freedesktop.Platform/x86_64/24.08
org.freedesktop.Platform runtime/org.freedesktop.Platform/x86_64/23.08
";
        assert_eq!(
            unused_refs(runtimes, apps),
            vec!["runtime/org.freedesktop.Platform/x86_64/23.08"]
        );
    }

    #[test]
    fn a_prefix_that_is_not_a_dotted_extension_does_not_match() {
        // `org.gnome.Platformer` is not an extension of `org.gnome.Platform`.
        assert!(extends("org.gnome.Platform.Locale", "org.gnome.Platform"));
        assert!(!extends("org.gnome.Platformer", "org.gnome.Platform"));
        assert!(!extends("org.gnome.Platform", "org.gnome.Platform"));
    }
}
